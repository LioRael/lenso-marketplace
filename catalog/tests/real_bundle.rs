//! Explicit real-artifact acceptance; run after CLI pack and private extraction.
use lenso_marketplace_catalog::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[test]
#[ignore = "requires MARKETPLACE_BUNDLE_DIRECTORY and MARKETPLACE_BUNDLE_ARCHIVE from CLI pack"]
fn verified_plugin_can_be_submitted_reviewed_and_discovered_after_restart() {
    let directory = PathBuf::from(
        std::env::var("MARKETPLACE_BUNDLE_DIRECTORY").expect("provide private extracted Bundle"),
    );
    let archive =
        std::fs::read(std::env::var("MARKETPLACE_BUNDLE_ARCHIVE").expect("provide CLI archive"))
            .unwrap();
    let verified = lenso_plugin_bundle::verify_bundle_directory(&directory).unwrap();
    let release = Release {
        plugin_id: verified.plugin_id,
        version: verified.release_version,
        publisher_id: "test-publisher".into(),
        title: "Echo".into(),
        summary: "Return input through a real Process Plugin".into(),
        description: String::new(),
        presentation: Some(Presentation {
            icon_url: Some("https://example.test/icon.png".into()),
            screenshots: vec![Screenshot {
                url: "https://example.test/screen.png".into(),
                caption: "Echo tool result".into(),
            }],
            getting_started: "Send a non-empty text argument.".into(),
        }),
        source_url: "https://example.test/source".into(),
        source_revision: "a".repeat(40),
        license: "MIT".into(),
        artifact: Artifact {
            url: "https://example.test/echo.lenso-plugin".into(),
            digest: digest(&archive),
            size: archive.len() as u64,
            manifest_digest: verified.manifest_digest,
        },
        availability: Availability::Listed,
    };
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("catalog.db");
    let reviewers = BTreeSet::from(["reviewer".into()]);
    let mut store = directory::Directory::open(&path, "test", reviewers.clone()).unwrap();
    store
        .claim_namespace(
            "reviewer",
            "lenso.marketplace",
            "test-publisher",
            "author",
            100,
        )
        .unwrap();
    let id = store
        .submit("author", &release, &archive, &directory, 101)
        .unwrap();
    let (_, proposal, _) = store.inspect_submission("reviewer", id).unwrap();
    store
        .approve("reviewer", id, &proposal, "fixture-review-v1", 102)
        .unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
    store
        .publish("reviewer", 0, 103, 200, "test-key", &key)
        .unwrap();
    drop(store);
    let store = directory::Directory::open(&path, "test", reviewers).unwrap();
    let trust = Trust {
        catalog_id: "test".into(),
        keys: BTreeMap::from([("test-key".into(), key.verifying_key())]),
    };
    let catalog = verify(&store.latest().unwrap().unwrap(), &trust, None, 150).unwrap();
    let selected = catalog
        .select(&release.plugin_id, &release.version, 150)
        .unwrap();
    assert_eq!(selected.presentation, release.presentation);
    selected.verify_archive_bytes(&archive).unwrap();
    selected.verify_bundle_directory(&directory).unwrap();
    let mut wrong = selected.clone();
    wrong.plugin_id = "lenso.marketplace.other".into();
    assert!(wrong.verify_bundle_directory(&directory).is_err());
}
