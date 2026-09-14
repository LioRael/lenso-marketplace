//! Explicit real-artifact acceptance; run after CLI pack and private extraction.
use lenso_marketplace_catalog::*;
use std::{collections::BTreeSet, path::PathBuf};

fn main() {
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
        summary: "Check a plugin connection by sending text and reading it back.".into(),
        description: "Echo returns the text you send it, unchanged. Use it to check that a Lenso Host can call a Process Plugin and receive its response.\n\nExample: send {\"text\":\"hello\"}. The response content is hello, with text content type.\n\nPass a non-empty UTF-8 string in the text argument. The tool returns that string as text content. Empty input is rejected; the input schema limits text to 4,096 characters.\n\nEcho runs as a separate native process using the Lenso tool SDK. It is a development fixture, with no external service or account to configure.".into(),
        presentation: Some(Presentation { getting_started: "Call the Echo tool with a non-empty text argument, for example {\"text\":\"hello\"}. The response returns the same text. No account or external service is required.".into(), ..Presentation::default() }),
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
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: seed_fixture NEW_DATABASE"),
    );
    assert!(!path.exists(), "use a new test database");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let reviewers = BTreeSet::from(["reviewer".into()]);
    let mut store = directory::Directory::open(&path, "local-fixture", reviewers.clone()).unwrap();
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
        .publish("reviewer", 0, now, now + 3600, "test-key", &key)
        .unwrap();

    println!("{}", hex::encode(key.verifying_key().to_bytes()));
}
