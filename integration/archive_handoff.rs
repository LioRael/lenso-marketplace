//! Source integration proof for the pending lenso-cli verified archive API.
//! Compiled only by verify-archive-handoff.sh against an explicit CLI checkout.
#[cfg(test)]
mod tests {
    use lenso_app_authoring::bundle_archive::{
        PluginArchiveIdentity, PluginReleaseIdentity, VerifiedPluginArchive,
    };
    use lenso_marketplace_catalog::{
        Artifact, Availability, Release, digest, directory::Directory,
    };
    use std::{collections::BTreeSet, fs};

    #[test]
    fn ingestion_binds_transport_bundle_and_publisher_identity() {
        let bytes = fs::read(std::env::var("MARKETPLACE_BUNDLE_ARCHIVE").unwrap()).unwrap();
        let identity = PluginArchiveIdentity {
            size: bytes.len() as u64,
            sha256: digest(&bytes),
        };
        let verified = VerifiedPluginArchive::read(bytes.as_slice(), &identity).unwrap();
        let bundle = verified.bundle();
        let release = Release {
            plugin_id: bundle.plugin_id.clone(),
            version: bundle.release_version.clone(),
            publisher_id: "fixture".into(),
            title: "Echo".into(),
            summary: "Fixture for verified archive ingestion".into(),
            description: String::new(),
            presentation: None,
            source_url: "https://example.test/source".into(),
            source_revision: "a".repeat(40),
            license: "MIT".into(),
            availability: Availability::Listed,
            artifact: Artifact {
                url: "https://example.test/echo.lenso-plugin".into(),
                size: identity.size,
                digest: identity.sha256,
                manifest_digest: bundle.manifest_digest.clone(),
            },
        };
        let root = tempfile::tempdir().unwrap();
        let mut directory = Directory::open(
            &root.path().join("directory.db"),
            "fixture",
            BTreeSet::from(["reviewer".into()]),
        )
        .unwrap();
        directory
            .claim_namespace("reviewer", "lenso.marketplace", "fixture", "author", 100)
            .unwrap();
        let id = directory
            .submit("author", &release, &bytes, &verified.directory(), 101)
            .unwrap();
        assert_eq!(
            directory.inspect_submission("author", id).unwrap().2,
            "awaiting_review"
        );
        let (_, proposal, _) = directory.inspect_submission("reviewer", id).unwrap();
        directory
            .approve("reviewer", id, &proposal, "fixture-v1", 102)
            .unwrap();
        let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
        let envelope = directory
            .publish("reviewer", 0, 103, 200, "test-key", &key)
            .unwrap();
        let trust = lenso_marketplace_catalog::Trust {
            catalog_id: "fixture".into(),
            keys: std::collections::BTreeMap::from([("test-key".into(), key.verifying_key())]),
        };
        let catalog = lenso_marketplace_catalog::verify(&envelope, &trust, None, 150).unwrap();
        let selected = catalog
            .select(&release.plugin_id, &release.version, 150)
            .unwrap();
        let admitted = VerifiedPluginArchive::read_release(
            bytes.as_slice(),
            &PluginArchiveIdentity {
                size: selected.artifact.size,
                sha256: selected.artifact.digest.clone(),
            },
            &PluginReleaseIdentity {
                plugin_id: selected.plugin_id.clone(),
                release_version: selected.version.clone(),
                manifest_digest: selected.artifact.manifest_digest.clone(),
            },
        )
        .unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::create_dir(target.path().join(".lenso")).unwrap();
        let host = lenso_app_plan::authoring::HostCatalog::new(
            [lenso_app_plan::authoring::HostSlot::many("tool-providers")],
            [],
            [],
        );
        fs::write(
            target.path().join(".lenso/host-catalog.json"),
            serde_json::to_vec(&host).unwrap(),
        )
        .unwrap();
        let candidate = admitted
            .prepare_mutation(target.path(), lenso_app_authoring::BundleMutation::Add)
            .unwrap();
        assert!(
            !target
                .path()
                .join("plugins/lenso.marketplace.echo/plugin.lenso-plugin")
                .exists()
        );
        assert_eq!(
            candidate.verified().manifest_digest,
            selected.artifact.manifest_digest
        );
        candidate.commit().unwrap();
        assert!(
            target
                .path()
                .join("plugins/lenso.marketplace.echo/plugin.lenso-plugin/lenso-plugin.json")
                .is_file()
        );
        assert!(
            catalog
                .select(&release.plugin_id, &release.version, 200)
                .is_err()
        );
        let mut wrong = release;
        wrong.plugin_id = "lenso.marketplace.other".into();
        assert!(
            directory
                .submit("author", &wrong, &bytes, &verified.directory(), 101)
                .is_err()
        );
    }
}
