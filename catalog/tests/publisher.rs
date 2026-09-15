#![cfg(feature = "native")]
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
    let trust = lenso_marketplace_catalog::Trust {
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
    let verified = lenso_marketplace_catalog::verify(
        receipt["envelope"].as_str().unwrap().as_bytes(),
        &trust,
        None,
        now,
    )
    .unwrap();
    assert_eq!(verified.snapshot().revision, 2);
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
    let recovered = lenso_marketplace_catalog::verify(
        receipt["envelope"].as_str().unwrap().as_bytes(),
        &trust,
        None,
        now,
    )
    .unwrap();
    assert_eq!(recovered.snapshot().revision, 3);
}
