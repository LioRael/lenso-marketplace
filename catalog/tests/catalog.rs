use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::SigningKey;
use lenso_marketplace_catalog::*;
use std::collections::BTreeMap;

fn fixture() -> (Snapshot, SigningKey, Trust) {
    let key = SigningKey::from_bytes(&[17; 32]);
    let trust = Trust {
        catalog_id: "official-test".into(),
        keys: BTreeMap::from([("test-key".into(), key.verifying_key())]),
    };
    let snapshot = Snapshot::new(
        "official-test".into(),
        4,
        100,
        200,
        vec![Release {
            plugin_id: "lenso.marketplace.echo".into(),
            version: "0.1.0".into(),
            publisher_id: "lenso".into(),
            title: "Echo".into(),
            summary: "Return input text".into(),
            description: String::new(),
            presentation: None,
            source_url: "https://example.test/source".into(),
            source_revision: "a".repeat(40),
            license: "MIT".into(),
            artifact: Artifact {
                url: "https://example.test/echo.lenso-plugin".into(),
                digest: digest(b"archive"),
                size: 7,
                manifest_digest: digest(b"manifest"),
            },
            availability: Availability::Listed,
        }],
    );
    (snapshot, key, trust)
}

#[test]
fn exact_release_is_searchable_but_not_an_installation_grant() {
    let (snapshot, key, trust) = fixture();
    let bytes = sign(&snapshot, "test-key", &key).unwrap();
    let verified = verify(&bytes, &trust, None, 150).unwrap();
    assert_eq!(verified.search("echo", 0, 20).unwrap().len(), 1);
    let release = verified
        .select("lenso.marketplace.echo", "0.1.0", 150)
        .unwrap();
    release.verify_archive_bytes(b"archive").unwrap();
    assert!(release.verify_archive_bytes(b"changed").is_err());
    assert!(
        verified
            .select("lenso.marketplace.echo", "latest", 150)
            .is_err()
    );
    assert!(
        verified
            .select("lenso.marketplace.echo", "0.1.0", 200)
            .is_err()
    );
}

#[test]
fn unknown_keys_modified_payload_and_wrong_catalog_fail_closed() {
    let (snapshot, key, trust) = fixture();
    let bytes = sign(&snapshot, "test-key", &key).unwrap();
    let mut envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
    envelope.payload_base64 = STANDARD.encode(b"{}");
    assert!(verify(&serde_json::to_vec(&envelope).unwrap(), &trust, None, 150).is_err());
    assert!(
        verify(
            &sign(&snapshot, "other-key", &key).unwrap(),
            &trust,
            None,
            150
        )
        .is_err()
    );
    let wrong_trust = Trust {
        catalog_id: "different".into(),
        ..trust
    };
    assert!(verify(&bytes, &wrong_trust, None, 150).is_err());
}

#[test]
fn checkpoints_reject_rollback_and_same_revision_equivocation() {
    let (mut snapshot, key, trust) = fixture();
    let bytes = sign(&snapshot, "test-key", &key).unwrap();
    let accepted = verify(&bytes, &trust, None, 150).unwrap();
    verify(&bytes, &trust, Some(accepted.checkpoint()), 150).unwrap();
    snapshot.revision -= 1;
    assert!(
        verify(
            &sign(&snapshot, "test-key", &key).unwrap(),
            &trust,
            Some(accepted.checkpoint()),
            150
        )
        .is_err()
    );
    snapshot.revision += 1;
    snapshot.releases[0].title = "Changed at same revision".into();
    assert!(
        verify(
            &sign(&snapshot, "test-key", &key).unwrap(),
            &trust,
            Some(accepted.checkpoint()),
            150
        )
        .is_err()
    );
    snapshot.revision += 1;
    verify(
        &sign(&snapshot, "test-key", &key).unwrap(),
        &trust,
        Some(accepted.checkpoint()),
        150,
    )
    .unwrap();
}

#[test]
fn invalid_releases_and_time_windows_cannot_be_published() {
    let (snapshot, key, trust) = fixture();
    let bytes = sign(&snapshot, "test-key", &key).unwrap();
    for now in [99, 200, 201] {
        assert!(verify(&bytes, &trust, None, now).is_err());
    }
    for id in ["single", "a.9bad", "a.bad-", "a..b"] {
        let mut invalid = snapshot.clone();
        invalid.releases[0].plugin_id = id.into();
        assert!(sign(&invalid, "test-key", &key).is_err());
    }
    let mut duplicate = snapshot.clone();
    duplicate.releases.push(duplicate.releases[0].clone());
    assert!(sign(&duplicate, "test-key", &key).is_err());
    let mut invalid = snapshot.clone();
    invalid.releases[0].artifact.url = "https://user:secret@example.test/bundle".into();
    assert!(sign(&invalid, "test-key", &key).is_err());
    let mut invalid = snapshot;
    invalid.expires_at = u64::MAX;
    assert!(sign(&invalid, "test-key", &key).is_err());
}

#[test]
fn yanked_and_revoked_releases_remain_visible_in_history_but_cannot_be_selected() {
    let (mut snapshot, key, trust) = fixture();
    for state in [Availability::Yanked, Availability::Revoked] {
        snapshot.releases[0].availability = state;
        let verified = verify(
            &sign(&snapshot, "test-key", &key).unwrap(),
            &trust,
            None,
            150,
        )
        .unwrap();
        assert_eq!(verified.snapshot().releases.len(), 1);
        assert!(verified.search("echo", 0, 20).unwrap().is_empty());
        assert!(
            verified
                .select("lenso.marketplace.echo", "0.1.0", 150)
                .is_err()
        );
    }
}

#[test]
fn search_and_untrusted_envelope_allocation_are_bounded() {
    let (snapshot, key, trust) = fixture();
    let verified = verify(
        &sign(&snapshot, "test-key", &key).unwrap(),
        &trust,
        None,
        150,
    )
    .unwrap();
    assert!(verified.search(&"x".repeat(257), 0, 10).is_err());
    assert!(verified.search("", 0, 101).is_err());
    assert!(verified.search("", usize::MAX, 10).unwrap().is_empty());
    assert!(verify(&vec![0; MAX_ENVELOPE_BYTES + 1], &trust, None, 150).is_err());
}

#[test]
fn published_identity_cannot_be_replaced_in_later_snapshots() {
    let (mut snapshot, key, trust) = fixture();
    let first = verify(
        &sign(&snapshot, "test-key", &key).unwrap(),
        &trust,
        None,
        150,
    )
    .unwrap();
    snapshot.revision += 1;
    snapshot.releases[0].artifact.digest = digest(b"replacement");
    assert!(
        verify(
            &sign(&snapshot, "test-key", &key).unwrap(),
            &trust,
            Some(first.checkpoint()),
            150
        )
        .is_err()
    );
    // Omitting a release cannot erase the accepted identity history.
    let removed = Snapshot {
        releases: vec![],
        ..snapshot.clone()
    };
    let second = verify(
        &sign(&removed, "test-key", &key).unwrap(),
        &trust,
        Some(first.checkpoint()),
        150,
    )
    .unwrap();
    snapshot.revision += 1;
    assert!(
        verify(
            &sign(&snapshot, "test-key", &key).unwrap(),
            &trust,
            Some(second.checkpoint()),
            150
        )
        .is_err()
    );
}

#[test]
fn rust_accepts_the_committed_cross_language_vector() {
    let vector: serde_json::Value = serde_json::from_str(include_str!("conformance.json")).unwrap();
    let public: [u8; 32] = hex::decode(vector["public_key_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let trust = Trust {
        catalog_id: "conformance-only".into(),
        keys: BTreeMap::from([(
            "test-key".into(),
            ed25519_dalek::VerifyingKey::from_bytes(&public).unwrap(),
        )]),
    };
    verify(
        &serde_json::to_vec(&vector["envelope"]).unwrap(),
        &trust,
        None,
        150,
    )
    .unwrap();
}

// Publisher text is untrusted input: keep old catalogs compatible and reject
// oversized/control-bearing descriptions before they enter a signed snapshot.
#[test]
fn descriptions_are_optional_bounded_and_signed() {
    let (mut snapshot, key, trust) = fixture();
    let old = serde_json::to_value(&snapshot.releases[0]).unwrap();
    assert!(old.get("description").is_none());
    let decoded: Release = serde_json::from_value(old).unwrap();
    assert!(decoded.description.is_empty());
    snapshot.releases[0].description =
        "First paragraph.\n\n第二段。 <script>plain text</script>".into();
    let bytes = sign(&snapshot, "test-key", &key).unwrap();
    let verified = verify(&bytes, &trust, None, 150).unwrap();
    assert_eq!(
        verified
            .select("lenso.marketplace.echo", "0.1.0", 150)
            .unwrap()
            .description,
        snapshot.releases[0].description
    );
    snapshot.releases[0].description = "x".repeat(16_385);
    assert!(sign(&snapshot, "test-key", &key).is_err());
    snapshot.releases[0].description = "bad\0text".into();
    assert!(sign(&snapshot, "test-key", &key).is_err());
}

#[test]
fn presentation_is_optional_signed_and_round_trips() {
    let (mut snapshot, key, trust) = fixture();
    let old = serde_json::to_value(&snapshot.releases[0]).unwrap();
    assert!(old.get("presentation").is_none());
    assert!(
        serde_json::from_value::<Release>(old)
            .unwrap()
            .presentation
            .is_none()
    );
    snapshot.releases[0].presentation = Some(Presentation {
        icon_url: Some("https://example.test/icon.png".into()),
        screenshots: vec![Screenshot {
            url: "https://example.test/screen.png".into(),
            caption: "The main workspace".into(),
        }],
        getting_started: "Open the workspace.\n\n<script>plain text</script>".into(),
    });
    let bytes = sign(&snapshot, "test-key", &key).unwrap();
    let verified = verify(&bytes, &trust, None, 150).unwrap();
    assert_eq!(
        verified.snapshot().releases[0].presentation,
        snapshot.releases[0].presentation
    );
    let mut envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
    let mut payload: serde_json::Value =
        serde_json::from_slice(&STANDARD.decode(&envelope.payload_base64).unwrap()).unwrap();
    payload["releases"][0]["presentation"]["screenshots"][0]["url"] =
        "https://example.test/replaced.png".into();
    envelope.payload_base64 = STANDARD.encode(serde_json::to_vec(&payload).unwrap());
    assert!(verify(&serde_json::to_vec(&envelope).unwrap(), &trust, None, 150).is_err());
}

#[test]
fn presentation_rejects_unsafe_urls_and_unbounded_publisher_input() {
    let (mut snapshot, key, _) = fixture();
    let screenshot = Screenshot {
        url: "https://example.test/screen.png".into(),
        caption: "Workspace".into(),
    };
    let mut invalid = vec![];
    for url in [
        "http://example.test/icon.png",
        "javascript:alert(1)",
        "data:image/png;base64,AA==",
        "https://user:secret@example.test/icon.png",
        "https://example.test/icon.png#fragment",
    ] {
        invalid.push(Presentation {
            icon_url: Some(url.into()),
            ..Presentation::default()
        });
        invalid.push(Presentation {
            screenshots: vec![Screenshot {
                url: url.into(),
                ..screenshot.clone()
            }],
            ..Presentation::default()
        });
    }
    invalid.push(Presentation {
        screenshots: vec![screenshot.clone(); 7],
        ..Presentation::default()
    });
    invalid.push(Presentation {
        screenshots: vec![screenshot.clone(); 2],
        ..Presentation::default()
    });
    for caption in ["".into(), "x".repeat(321), "bad\0caption".into()] {
        invalid.push(Presentation {
            screenshots: vec![Screenshot {
                caption,
                ..screenshot.clone()
            }],
            ..Presentation::default()
        });
    }
    for getting_started in ["x".repeat(16_385), "bad\0guide".into()] {
        invalid.push(Presentation {
            getting_started,
            ..Presentation::default()
        });
    }
    for presentation in invalid {
        snapshot.releases[0].presentation = Some(presentation);
        assert!(sign(&snapshot, "test-key", &key).is_err());
    }
}
