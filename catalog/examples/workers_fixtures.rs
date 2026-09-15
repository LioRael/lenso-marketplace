//! Public test key and synthetic catalog, exclusively for isolated G3 proof resources.
use lenso_marketplace_catalog::{Artifact, Availability, Release, Snapshot, digest, sign};
fn main() -> anyhow::Result<()> {
    let output = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&output)?;
    let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let release = Release {
        plugin_id: "lenso.marketplace.echo".into(),
        version: "0.1.0".into(),
        publisher_id: "lenso".into(),
        title: "Echo".into(),
        summary: "G3 signed identity proof".into(),
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
    };
    for (name, revision, issued, expiry) in [
        ("first", 1, now - 10, now + 3600),
        ("second", 2, now - 10, now + 3600),
        ("equivocation", 1, now - 9, now + 3600),
        ("expired", 3, now - 100, now - 50),
    ] {
        let bytes = sign(
            &Snapshot::new(
                "workers-g3-proof".into(),
                revision,
                issued,
                expiry,
                if name == "second" {
                    vec![]
                } else {
                    vec![release.clone()]
                },
            ),
            "test-key",
            &key,
        )?;
        std::fs::write(output.join(format!("{name}.json")), bytes)?;
    }
    let mut large = vec![release.clone()];
    for index in 0..160 {
        let mut entry = release.clone();
        entry.plugin_id = format!("lenso.large.p{index}");
        entry.description = "x".repeat(16 * 1024);
        large.push(entry);
    }
    std::fs::write(
        output.join("large.json"),
        sign(
            &Snapshot::new("workers-g3-proof".into(), 4, now - 10, now + 3600, large),
            "test-key",
            &key,
        )?,
    )?;
    let mut changed = release;
    changed.artifact.digest = digest(b"different artifact");
    std::fs::write(
        output.join("identity-change.json"),
        sign(
            &Snapshot::new(
                "workers-g3-proof".into(),
                3,
                now - 10,
                now + 3600,
                vec![changed],
            ),
            "test-key",
            &key,
        )?,
    )?;
    std::fs::write(
        output.join("restored.json"),
        sign(
            &Snapshot::new("workers-g3-proof".into(), 5, now - 10, now + 3600, vec![]),
            "test-key",
            &key,
        )?,
    )?;
    let mut tampered: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("first.json"))?)?;
    tampered["signature_base64"] = serde_json::Value::String("A".repeat(86) + "==");
    std::fs::write(output.join("tampered.json"), serde_json::to_vec(&tampered)?)?;
    Ok(())
}
