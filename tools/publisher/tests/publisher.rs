use ed25519_dalek::SigningKey;
use std::{
    fs,
    io::Write,
    process::{Command, Output, Stdio},
};

fn invoke(config: &std::path::Path, args: &[&str], key: Option<&[u8]>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lenso-marketplace-publisher"))
        .arg(config)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(key) = key {
        child.stdin.take().unwrap().write_all(key).unwrap();
    }
    child.wait_with_output().unwrap()
}

#[test]
fn operator_preserves_committed_bytes_and_rejects_stale_publication() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("operator.json");
    let database = home.path().join("publisher.sqlite3");
    let key = [19u8; 32];
    fs::write(&config, serde_json::to_vec(&serde_json::json!({
        "database": database, "catalog_id": "operator-test", "reviewers": ["reviewer"],
        "key_id": "operator-key", "public_key_hex": hex::encode(SigningKey::from_bytes(&key).verifying_key().as_bytes())
    })).unwrap()).unwrap();
    assert!(
        !invoke(&config, &["publish", "reviewer", "0", "3600"], Some(&key))
            .status
            .success()
    );
    assert!(!database.exists());
    assert!(invoke(&config, &["initialize"], None).status.success());
    assert!(!invoke(&config, &["initialize"], None).status.success());
    assert!(
        !invoke(&config, &["publish", "stranger", "0", "3600"], None)
            .status
            .success()
    );
    assert!(
        !invoke(
            &config,
            &["publish", "reviewer", "0", "3600"],
            Some(&[20; 32])
        )
        .status
        .success()
    );
    assert!(!invoke(&config, &["export"], None).status.success());
    let first = invoke(&config, &["publish", "reviewer", "0", "3600"], Some(&key));
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        !invoke(&config, &["publish", "reviewer", "0", "3600"], Some(&key))
            .status
            .success()
    );
    assert_eq!(first.stdout, invoke(&config, &["export"], None).stdout);
    let next = invoke(&config, &["publish", "reviewer", "1", "3600"], Some(&key));
    assert!(next.status.success());
    assert_ne!(first.stdout, next.stdout);
    let receipt: serde_json::Value = serde_json::from_slice(&next.stdout).unwrap();
    let trust = lenso_plugin_catalog::Trust {
        catalog_id: "operator-test".into(),
        keys: std::collections::BTreeMap::from([(
            "operator-key".into(),
            SigningKey::from_bytes(&key).verifying_key(),
        )]),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let verified = lenso_plugin_catalog::verify(
        receipt["envelope"].as_str().unwrap().as_bytes(),
        &trust,
        None,
        now,
    )
    .unwrap();
    assert_eq!(verified.snapshot().revision, 2);
    let checked = invoke(
        &config,
        &["verify"],
        Some(receipt["envelope"].as_str().unwrap().as_bytes()),
    );
    assert!(checked.status.success());
    let checked: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(checked["revision"], 2);
    assert_eq!(checked["catalog_id"], "operator-test");
    assert!(!invoke(&config, &["verify"], Some(b"{}")).status.success());

    // A live WAL image cannot be backed up by copying only the main file.
    // Keep the connection open with auto-checkpoint disabled while backing up.
    let wal = rusqlite::Connection::open(&database).unwrap();
    wal.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE backup_marker(value TEXT); INSERT INTO backup_marker VALUES('wal-only');").unwrap();
    let backup = home.path().join("backup.sqlite3");
    let backed_up = invoke(&config, &["backup", backup.to_str().unwrap()], None);
    assert!(
        backed_up.status.success(),
        "{}",
        String::from_utf8_lossy(&backed_up.stderr)
    );
    let before = fs::read(&backup).unwrap();
    assert!(
        !invoke(&config, &["backup", backup.to_str().unwrap()], None)
            .status
            .success()
    );
    assert_eq!(before, fs::read(&backup).unwrap());
    let restored = home.path().join("restored.json");
    let mut restored_config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    restored_config["database"] = serde_json::json!(backup);
    fs::write(&restored, serde_json::to_vec(&restored_config).unwrap()).unwrap();
    assert_eq!(next.stdout, invoke(&restored, &["export"], None).stdout);
    let image = rusqlite::Connection::open(&backup).unwrap();
    let marker: String = image
        .query_row("SELECT value FROM backup_marker", [], |row| row.get(0))
        .unwrap();
    assert_eq!(marker, "wal-only");
    let history: i64 = image
        .query_row("SELECT count(*) FROM snapshots", [], |row| row.get(0))
        .unwrap();
    assert_eq!(history, 2);
    drop(image);
    assert!(
        !invoke(&restored, &["publish", "reviewer", "1", "3600"], Some(&key))
            .status
            .success()
    );
    assert!(
        invoke(&restored, &["publish", "reviewer", "2", "3600"], Some(&key))
            .status
            .success()
    );
    assert_eq!(next.stdout, invoke(&config, &["export"], None).stdout);
    drop(wal);

    let mut interrupted = Command::new(env!("CARGO_BIN_EXE_lenso-marketplace-publisher"))
        .arg(&config)
        .args(["publish", "reviewer", "2", "3600"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(interrupted.stdout.take());
    interrupted.stdin.take().unwrap().write_all(&key).unwrap();
    assert!(!interrupted.wait().unwrap().success());
    let recovered = invoke(&config, &["export"], None);
    assert!(recovered.status.success());
    let receipt: serde_json::Value = serde_json::from_slice(&recovered.stdout).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let recovered = lenso_plugin_catalog::verify(
        receipt["envelope"].as_str().unwrap().as_bytes(),
        &trust,
        None,
        now,
    )
    .unwrap();
    assert_eq!(recovered.snapshot().revision, 3);
}

#[test]
fn namespace_commands_require_existing_catalog_and_reviewer() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("operator.json");
    fs::write(&config, serde_json::to_vec(&serde_json::json!({
        "database": root.path().join("db.sqlite3"), "catalog_id":"author-test",
        "reviewers":["reviewer"],"key_id":"key",
        "public_key_hex":hex::encode(SigningKey::from_bytes(&[19;32]).verifying_key().as_bytes())
    })).unwrap()).unwrap();
    assert!(
        !invoke(
            &config,
            &["claim", "reviewer", "example", "publisher", "author"],
            None
        )
        .status
        .success()
    );
    assert!(invoke(&config, &["initialize"], None).status.success());
    assert!(
        !invoke(
            &config,
            &["claim", "stranger", "example", "publisher", "author"],
            None
        )
        .status
        .success()
    );
    assert!(
        invoke(
            &config,
            &["claim", "reviewer", "example", "publisher", "author"],
            None
        )
        .status
        .success()
    );
    assert!(
        !invoke(
            &config,
            &["claim", "reviewer", "example", "publisher", "other"],
            None
        )
        .status
        .success()
    );
}

#[test]
#[ignore = "requires two real CLI-built Echo archives via MARKETPLACE_AUTHOR_ARCHIVE and MARKETPLACE_AUTHOR_UPDATE"]
fn author_submission_review_and_update() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("operator.json");
    let key = [19; 32];
    fs::write(&config,serde_json::to_vec(&serde_json::json!({
        "database":root.path().join("db.sqlite3"),"catalog_id":"author-test","reviewers":["reviewer"],
        "key_id":"key","public_key_hex":hex::encode(SigningKey::from_bytes(&key).verifying_key().as_bytes())
    })).unwrap()).unwrap();
    assert!(invoke(&config, &["initialize"], None).status.success());
    assert!(
        invoke(
            &config,
            &[
                "claim",
                "reviewer",
                "lenso.marketplace",
                "publisher",
                "author"
            ],
            None
        )
        .status
        .success()
    );
    let metadata = root.path().join("metadata.json");
    fs::write(
        &metadata,
        serde_json::to_vec(&serde_json::json!({
            "publisher_id":"publisher","title":"Echo","summary":"Echo text","license":"MIT",
            "source_url":"https://example.com/source","source_revision":"a".repeat(40),
            "artifact_url":"https://example.com/echo.lenso-plugin"
        }))
        .unwrap(),
    )
    .unwrap();
    for (revision, var) in ["MARKETPLACE_AUTHOR_ARCHIVE", "MARKETPLACE_AUTHOR_UPDATE"]
        .iter()
        .enumerate()
    {
        let archive = std::path::PathBuf::from(std::env::var(var).unwrap());
        let candidate = root.path().join(format!("candidate-{revision}"));
        let output = Command::new(env!("CARGO_BIN_EXE_lenso-marketplace-author"))
            .args([
                "prepare",
                archive.to_str().unwrap(),
                metadata.to_str().unwrap(),
                candidate.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let (release, _) = lenso_marketplace_publisher::check(&candidate).unwrap();
        assert!(lenso_marketplace_publisher::prepare(&archive, &metadata, &candidate).is_err());
        assert!(
            !invoke(
                &config,
                &["submit", "stranger", candidate.to_str().unwrap()],
                None
            )
            .status
            .success()
        );
        let submitted = invoke(
            &config,
            &["submit", "author", candidate.to_str().unwrap()],
            None,
        );
        assert!(
            submitted.status.success(),
            "{}",
            String::from_utf8_lossy(&submitted.stderr)
        );
        assert_eq!(
            submitted.stdout,
            invoke(
                &config,
                &["submit", "author", candidate.to_str().unwrap()],
                None
            )
            .stdout
        );
        let receipt: serde_json::Value = serde_json::from_slice(&submitted.stdout).unwrap();
        let id = receipt["submission_id"].to_string();
        assert!(
            !invoke(
                &config,
                &["approve", "reviewer", &id, "wrong", "review-v1"],
                None
            )
            .status
            .success()
        );
        assert!(
            invoke(
                &config,
                &[
                    "approve",
                    "reviewer",
                    &id,
                    receipt["proposal_digest"].as_str().unwrap(),
                    "review-v1"
                ],
                None
            )
            .status
            .success()
        );
        let publication = invoke(
            &config,
            &["publish", "reviewer", &revision.to_string(), "3600"],
            Some(&key),
        );
        assert!(
            publication.status.success(),
            "{}",
            String::from_utf8_lossy(&publication.stderr)
        );
        let published: serde_json::Value = serde_json::from_slice(&publication.stdout).unwrap();
        assert!(
            invoke(
                &config,
                &["verify"],
                Some(published["envelope"].as_str().unwrap().as_bytes())
            )
            .status
            .success()
        );
        let trust = lenso_plugin_catalog::Trust {
            catalog_id: "author-test".into(),
            keys: std::collections::BTreeMap::from([(
                "key".into(),
                SigningKey::from_bytes(&key).verifying_key(),
            )]),
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let verified = lenso_plugin_catalog::verify(
            published["envelope"].as_str().unwrap().as_bytes(),
            &trust,
            None,
            now,
        )
        .unwrap();
        assert_eq!(verified.snapshot().releases.len(), revision + 1);
        assert!(
            verified
                .snapshot()
                .releases
                .iter()
                .any(|item| item.version == release.version)
        );
        assert!(
            !invoke(&config, &["inspect", "stranger", &id], None)
                .status
                .success()
        );
        let mut changed = release.clone();
        changed.title = "Changed immutable release".into();
        fs::write(
            candidate.join("release.json"),
            serde_json::to_vec(&changed).unwrap(),
        )
        .unwrap();
        assert!(
            !invoke(
                &config,
                &["submit", "author", candidate.to_str().unwrap()],
                None
            )
            .status
            .success()
        );
        fs::write(candidate.join("plugin.lenso-plugin"), b"tampered").unwrap();
        assert!(lenso_marketplace_publisher::check(&candidate).is_err());
    }
}

#[test]
fn author_rejects_invalid_archive_without_creating_output() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("bad.lenso-plugin");
    let metadata = root.path().join("metadata.json");
    let output = root.path().join("submission");
    fs::write(&archive, b"not an archive").unwrap();
    fs::write(
        &metadata,
        serde_json::to_vec(&serde_json::json!({
            "publisher_id":"example", "title":"Example", "summary":"Example plugin",
            "source_url":"https://example.com/source", "source_revision":"a".repeat(40),
            "license":"MIT", "artifact_url":"https://example.com/plugin"
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(lenso_marketplace_publisher::prepare(&archive, &metadata, &output).is_err());
    assert!(!output.exists());
    assert!(
        Command::new(env!("CARGO_BIN_EXE_lenso-marketplace-author"))
            .arg("--help")
            .output()
            .unwrap()
            .status
            .success()
    );
}
