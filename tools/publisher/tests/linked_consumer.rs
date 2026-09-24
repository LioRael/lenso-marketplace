use ed25519_dalek::SigningKey;
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

fn publisher(config: &Path, args: &[&str], input: Option<&[u8]>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lenso-marketplace-publisher"))
        .arg(config)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child.stdin.take().unwrap().write_all(input).unwrap();
    }
    child.wait_with_output().unwrap()
}

fn cli(cli: &Path, args: &[&str]) -> Output {
    Command::new(cli).args(args).output().unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn native_target() -> String {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        (arch @ ("aarch64" | "x86_64"), "macos") => format!("{arch}-apple-darwin"),
        (arch @ ("aarch64" | "x86_64"), "linux") => format!("{arch}-unknown-linux-gnu"),
        (arch, os) => panic!("unsupported local linked acceptance target: {arch}/{os}"),
    }
}

#[test]
#[ignore = "set LENSO_REAL_LINKED_CRATE_ARCHIVE to the cached published Secrets Env 0.1.8 .crate and LENSO_CANDIDATE_LENSO_CLI to a matching local CLI"]
fn real_crate_follows_reviewed_snapshot_into_exact_cli_adoption() {
    let crate_archive = std::env::var("LENSO_REAL_LINKED_CRATE_ARCHIVE").unwrap();
    let candidate_cli = std::env::var("LENSO_CANDIDATE_LENSO_CLI").unwrap();
    let archive = fs::read(&crate_archive).unwrap();
    assert_eq!(
        lenso_plugin_catalog::digest(&archive),
        "sha256:33e13b670163023baf390c9c0588d4f5f43905a6906690570ae11032f359a9ca"
    );

    let home = tempfile::tempdir().unwrap();
    let archive_path = home.path().join("lenso-secrets-env-plugin-0.1.8.crate");
    let metadata_path = home.path().join("linked-metadata.json");
    let document_path = home.path().join("readme.md");
    let prepared_path = home.path().join("prepared");
    let config_path = home.path().join("operator.json");
    let snapshot_path = home.path().join("snapshot.json");
    let trust_path = home.path().join("trust.json");
    let app_path = home.path().join("app");
    let key = [91u8; 32];
    let verifying_key = SigningKey::from_bytes(&key).verifying_key();
    let document = b"# Secrets Env\n\nExact local acceptance document.\n";
    fs::write(&archive_path, &archive).unwrap();
    fs::write(&document_path, document).unwrap();
    fs::write(
        &metadata_path,
        serde_json::to_vec(&serde_json::json!({
            "publisher_id": "lenso",
            "title": "Secrets Env",
            "summary": "Environment-backed secrets for a linked Host",
            "source_url": "https://github.com/LioRael/lenso-secrets-plugin",
            "source_revision": "a".repeat(40),
            "license": "MIT",
            "registry_url": "https://crates.io",
            "integration": "linked_plugin",
            "targets": [native_target()],
            "documentation": [{
                "id": "readme",
                "revision": "local-rev-1",
                "language": "en",
                "topic": "usage",
                "url": "https://example.test/docs/lenso.secrets.env/0.1.8/readme/local-rev-1.md",
                "digest": lenso_plugin_catalog::digest(document),
                "size": document.len(),
                "media_type": "text/markdown"
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let release = lenso_marketplace_publisher::prepare_linked_cargo(
        &archive_path,
        &metadata_path,
        &prepared_path,
    )
    .unwrap();
    assert_eq!(release.plugin_id, "lenso.secrets.env");
    assert_eq!(release.version, "0.1.8");
    assert_eq!(release.package, "lenso-secrets-env-plugin");
    assert_eq!(
        lenso_marketplace_publisher::check_linked_cargo(&prepared_path)
            .unwrap()
            .crate_digest,
        release.crate_digest
    );

    fs::write(
        &config_path,
        serde_json::to_vec(&serde_json::json!({
            "database": home.path().join("publisher.sqlite3"),
            "catalog_id": "local-linked-acceptance",
            "reviewers": ["reviewer"],
            "key_id": "local-test-only",
            "public_key_hex": hex::encode(verifying_key.as_bytes())
        }))
        .unwrap(),
    )
    .unwrap();
    success(&publisher(&config_path, &["initialize"], None));
    success(&publisher(
        &config_path,
        &["claim", "reviewer", "lenso", "lenso", "author"],
        None,
    ));
    let submitted = publisher(
        &config_path,
        &[
            "submit-linked-cargo",
            "author",
            prepared_path.join("release.json").to_str().unwrap(),
            prepared_path.join("plugin.crate").to_str().unwrap(),
        ],
        None,
    );
    success(&submitted);
    let submitted: serde_json::Value = serde_json::from_slice(&submitted.stdout).unwrap();
    let id = submitted["submission_id"].as_i64().unwrap().to_string();
    let inspected = publisher(
        &config_path,
        &["inspect-linked-cargo", "reviewer", &id],
        None,
    );
    success(&inspected);
    let inspected: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(inspected["state"], "awaiting_review");
    success(&publisher(
        &config_path,
        &[
            "approve-linked-cargo",
            "reviewer",
            &id,
            submitted["proposal_digest"].as_str().unwrap(),
            "local-review",
        ],
        None,
    ));
    let published = publisher(
        &config_path,
        &["publish-linked-cargo", "reviewer", "0", "3600"],
        Some(&key),
    );
    success(&published);
    assert_eq!(
        published.stdout,
        publisher(&config_path, &["export-linked-cargo"], None).stdout
    );
    let published: serde_json::Value = serde_json::from_slice(&published.stdout).unwrap();
    let envelope = published["envelope"].as_str().unwrap();
    fs::write(&snapshot_path, envelope).unwrap();
    fs::write(
        &trust_path,
        serde_json::to_vec(&serde_json::json!({
            "catalog_id": "local-linked-acceptance",
            "key_id": "local-test-only",
            "public_key_hex": hex::encode(verifying_key.as_bytes())
        }))
        .unwrap(),
    )
    .unwrap();
    success(&publisher(
        &config_path,
        &["verify-linked-cargo"],
        Some(envelope.as_bytes()),
    ));

    let selected = cli(
        Path::new(&candidate_cli),
        &[
            "app",
            "linked-catalog",
            "secrets",
            "--linked-snapshot",
            snapshot_path.to_str().unwrap(),
            "--trust",
            trust_path.to_str().unwrap(),
            "--json",
        ],
    );
    success(&selected);
    let selected: serde_json::Value = serde_json::from_slice(&selected.stdout).unwrap();
    assert_eq!(selected["releases"][0]["plugin_id"], "lenso.secrets.env");
    assert_eq!(selected["releases"][0]["version"], "0.1.8");
    let read = cli(
        Path::new(&candidate_cli),
        &[
            "app",
            "linked-doc",
            "lenso.secrets.env@0.1.8",
            "readme",
            "--revision",
            "local-rev-1",
            "--linked-snapshot",
            snapshot_path.to_str().unwrap(),
            "--trust",
            trust_path.to_str().unwrap(),
            "--file",
            document_path.to_str().unwrap(),
            "--json",
        ],
    );
    success(&read);
    let read: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(read["content"], String::from_utf8_lossy(document).as_ref());
    assert_eq!(read["content_is_untrusted"], true);
    fs::write(&document_path, b"tampered local document").unwrap();
    let tampered_document = cli(
        Path::new(&candidate_cli),
        &[
            "app",
            "linked-doc",
            "lenso.secrets.env@0.1.8",
            "readme",
            "--revision",
            "local-rev-1",
            "--linked-snapshot",
            snapshot_path.to_str().unwrap(),
            "--trust",
            trust_path.to_str().unwrap(),
            "--file",
            document_path.to_str().unwrap(),
            "--json",
        ],
    );
    assert!(!tampered_document.status.success());
    fs::write(&document_path, document).unwrap();

    success(&cli(
        Path::new(&candidate_cli),
        &["app", "create", app_path.to_str().unwrap(), "--no-install"],
    ));
    let add = |version: &str, archive_path: &Path| {
        cli(
            Path::new(&candidate_cli),
            &[
                "app",
                "add",
                version,
                "--root",
                app_path.to_str().unwrap(),
                "--linked-snapshot",
                snapshot_path.to_str().unwrap(),
                "--trust",
                trust_path.to_str().unwrap(),
                "--crate",
                archive_path.to_str().unwrap(),
                "--no-install",
            ],
        )
    };
    assert!(
        !add("lenso.secrets.env@0.1.9", &archive_path)
            .status
            .success()
    );
    success(&add("lenso.secrets.env@0.1.8", &archive_path));
    success(&add("lenso.secrets.env@0.1.8", &archive_path));
    let discovered = cli(
        Path::new(&candidate_cli),
        &[
            "app",
            "discover",
            "--json",
            "--root",
            app_path.to_str().unwrap(),
        ],
    );
    success(&discovered);
    let discovered: serde_json::Value = serde_json::from_slice(&discovered.stdout).unwrap();
    assert_eq!(
        discovered["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|candidate| candidate["plugin_id"] == "lenso.secrets.env")
            .count(),
        1
    );
    success(&cli(
        Path::new(&candidate_cli),
        &[
            "app",
            "unadopt",
            "lenso.secrets.env@0.1.8",
            "--root",
            app_path.to_str().unwrap(),
        ],
    ));
    assert!(!app_path.join("plugins/lenso.secrets.env").exists());
}
