#![cfg(feature = "package-publication")]

use ed25519_dalek::SigningKey;
use lenso_plugin_catalog::{
    Availability, Distribution, DistributionKind, Trust, digest,
    package::{PackageRelease, verify},
};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::Path,
    process::{Command, Output, Stdio},
};

fn invoke(config: &Path, args: &[&str], input: Option<&[u8]>) -> Output {
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

fn archive_with_manifest(manifest: serde_json::Value) -> Vec<u8> {
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let manifest = serde_json::to_vec(&manifest).unwrap();
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "package/package.json", Cursor::new(manifest))
        .unwrap();
    let lock = b"{}\n";
    let mut header = tar::Header::new_gnu();
    header.set_size(lock.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "package/bun.lock", Cursor::new(lock))
        .unwrap();
    tar.into_inner().unwrap().finish().unwrap()
}

fn archive(name: &str, version: &str) -> Vec<u8> {
    archive_with_manifest(serde_json::json!({
        "name":name,
        "version":version,
        "lenso":{
            "pluginId":"example.notes",
            "releaseVersion":"1.2.3",
            "runtime":"bun",
            "rootSlot":"tools"
        }
    }))
}

fn release(integrity: &str) -> PackageRelease {
    PackageRelease {
        plugin_id: "example.notes".into(),
        version: "1.2.3".into(),
        publisher_id: "example-publisher".into(),
        title: "Notes".into(),
        summary: "Notes Plugin".into(),
        source_url: "https://example.test/source".into(),
        source_revision: "a".repeat(40),
        license: "MIT".into(),
        distributions: vec![Distribution {
            id: "npm".into(),
            kind: DistributionKind::NpmPackage,
            package: "@example/notes".into(),
            version: "4.5.6".into(),
            integrity: Some(integrity.into()),
            registry_url: Some("https://registry.npmjs.org".into()),
            artifact: None,
            targets: vec![],
        }],
        availability: Availability::Listed,
        documentation: vec![],
    }
}

#[test]
fn operator_reviews_and_signs_exact_npm_only_release() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("operator.json");
    let database = home.path().join("publisher.sqlite3");
    let release_path = home.path().join("package.json");
    let archive_path = home.path().join("package.tgz");
    let key = [23u8; 32];
    fs::write(
        &config,
        serde_json::to_vec(&serde_json::json!({
            "database":database,
            "catalog_id":"package-test",
            "reviewers":["reviewer"],
            "key_id":"operator-key",
            "public_key_hex":hex::encode(SigningKey::from_bytes(&key).verifying_key().as_bytes())
        }))
        .unwrap(),
    )
    .unwrap();
    let bytes = archive("@example/notes", "4.5.6");
    fs::write(&archive_path, &bytes).unwrap();
    let candidate = release(&digest(&bytes));
    fs::write(&release_path, serde_json::to_vec(&candidate).unwrap()).unwrap();
    assert!(invoke(&config, &["initialize"], None).status.success());
    assert!(
        invoke(
            &config,
            &[
                "claim",
                "reviewer",
                "example",
                "example-publisher",
                "author"
            ],
            None,
        )
        .status
        .success()
    );
    assert!(
        invoke(
            &config,
            &[
                "claim",
                "reviewer",
                "other",
                "example-publisher",
                "other-author"
            ],
            None,
        )
        .status
        .success()
    );
    let path = archive_path.to_str().unwrap();
    let release_file = release_path.to_str().unwrap();
    let non_plugin_archive = home.path().join("non-plugin.tgz");
    let non_plugin_bytes = archive_with_manifest(serde_json::json!({
        "name":"@example/notes",
        "version":"4.5.6"
    }));
    fs::write(&non_plugin_archive, &non_plugin_bytes).unwrap();
    fs::write(
        &release_path,
        serde_json::to_vec(&release(&digest(&non_plugin_bytes))).unwrap(),
    )
    .unwrap();
    assert!(
        !invoke(
            &config,
            &[
                "submit-package",
                "author",
                release_file,
                "npm",
                non_plugin_archive.to_str().unwrap()
            ],
            None,
        )
        .status
        .success()
    );
    fs::write(&release_path, serde_json::to_vec(&candidate).unwrap()).unwrap();
    assert!(
        !invoke(
            &config,
            &["submit-package", "stranger", release_file, "npm", path],
            None
        )
        .status
        .success()
    );

    let wrong_archive = home.path().join("wrong.tgz");
    let wrong_bytes = archive("@example/other", "4.5.6");
    fs::write(&wrong_archive, &wrong_bytes).unwrap();
    assert!(
        !invoke(
            &config,
            &[
                "submit-package",
                "author",
                release_file,
                "npm",
                wrong_archive.to_str().unwrap()
            ],
            None
        )
        .status
        .success()
    );
    fs::write(
        &release_path,
        serde_json::to_vec(&release(&digest(&wrong_bytes))).unwrap(),
    )
    .unwrap();
    assert!(
        !invoke(
            &config,
            &[
                "submit-package",
                "author",
                release_file,
                "npm",
                wrong_archive.to_str().unwrap()
            ],
            None,
        )
        .status
        .success()
    );
    fs::write(&release_path, serde_json::to_vec(&candidate).unwrap()).unwrap();
    assert!(
        !invoke(
            &config,
            &[
                "submit-package",
                "author",
                release_file,
                "npm",
                path,
                "extra",
                path
            ],
            None
        )
        .status
        .success()
    );

    let submitted = invoke(
        &config,
        &["submit-package", "author", release_file, "npm", path],
        None,
    );
    assert!(
        submitted.status.success(),
        "{}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&submitted.stdout).unwrap();
    let id = receipt["submission_id"].as_i64().unwrap().to_string();
    let proposal = receipt["proposal_digest"].as_str().unwrap();
    assert!(
        !invoke(&config, &["inspect-package", "other-author", &id], None)
            .status
            .success()
    );
    let again = invoke(
        &config,
        &["submit-package", "author", release_file, "npm", path],
        None,
    );
    assert!(again.status.success());
    assert_eq!(submitted.stdout, again.stdout);
    assert!(
        !invoke(
            &config,
            &["approve-package", "stranger", &id, proposal, "v1"],
            None
        )
        .status
        .success()
    );
    assert!(
        !invoke(
            &config,
            &["approve-package", "reviewer", &id, &"0".repeat(64), "v1"],
            None
        )
        .status
        .success()
    );
    assert!(
        invoke(
            &config,
            &["approve-package", "reviewer", &id, proposal, "v1"],
            None
        )
        .status
        .success()
    );
    assert!(
        !invoke(
            &config,
            &["publish-package", "reviewer", "0", "3600"],
            Some(&[1; 32])
        )
        .status
        .success()
    );
    let published = invoke(
        &config,
        &["publish-package", "reviewer", "0", "3600"],
        Some(&key),
    );
    assert!(
        published.status.success(),
        "{}",
        String::from_utf8_lossy(&published.stderr)
    );
    assert!(
        !invoke(
            &config,
            &["publish-package", "reviewer", "0", "3600"],
            Some(&key)
        )
        .status
        .success()
    );
    let exported = invoke(&config, &["export-package"], None);
    assert_eq!(published.stdout, exported.stdout);
    let publication: serde_json::Value = serde_json::from_slice(&published.stdout).unwrap();
    let envelope = publication["envelope"].as_str().unwrap();
    let trust = Trust {
        catalog_id: "package-test".into(),
        keys: BTreeMap::from([(
            "operator-key".into(),
            SigningKey::from_bytes(&key).verifying_key(),
        )]),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let verified = verify(envelope.as_bytes(), &trust, None, now).unwrap();
    assert_eq!(verified.snapshot().releases, vec![candidate]);
    assert_eq!(
        verified
            .select_npm("example.notes", "1.2.3", "npm", now)
            .unwrap()
            .integrity
            .as_deref(),
        Some(digest(&bytes).as_str())
    );
    let receipt = invoke(&config, &["verify-package"], Some(envelope.as_bytes()));
    assert!(receipt.status.success());
    let receipt: serde_json::Value = serde_json::from_slice(&receipt.stdout).unwrap();
    assert_eq!(receipt["documents"], serde_json::json!([]));
    assert!(
        !invoke(&config, &["verify-linked-cargo"], Some(envelope.as_bytes()))
            .status
            .success()
    );

    let backup = home.path().join("backup.sqlite3");
    let backed_up = invoke(&config, &["backup", backup.to_str().unwrap()], None);
    assert!(backed_up.status.success());
    let backed: serde_json::Value = serde_json::from_slice(&backed_up.stdout).unwrap();
    assert_eq!(backed["package_publication_digest"], publication["digest"]);
}

#[test]
fn package_identity_cannot_reuse_portable_or_linked_history() {
    for table in ["submissions", "linked_cargo_submissions"] {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("publisher.sqlite3");
        let mut directory = lenso_marketplace_directory_plugin::publishing::Directory::open(
            &database,
            "package-test",
            ["reviewer".into()].into(),
        )
        .unwrap();
        directory
            .claim_namespace("reviewer", "example", "example-publisher", "author", 100)
            .unwrap();
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection.execute(
            &format!("INSERT INTO {table}(identity,publisher,body,digest,state) VALUES(?1,?2,'legacy','legacy','published')"),
            rusqlite::params!["example.notes@1.2.3", "example-publisher"],
        ).unwrap();
        let bytes = archive("@example/notes", "4.5.6");
        let candidate = release(&digest(&bytes));
        let archives = BTreeMap::from([("npm".into(), bytes)]);
        assert!(
            directory
                .submit_package("author", &candidate, &archives, 101)
                .is_err()
        );
    }
}

#[test]
fn all_base_publishers_reject_cross_channel_identity_collision() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("publisher.sqlite3");
    let mut directory = lenso_marketplace_directory_plugin::publishing::Directory::open(
        &database,
        "package-test",
        ["reviewer".into()].into(),
    )
    .unwrap();
    directory
        .claim_namespace("reviewer", "example", "example-publisher", "author", 100)
        .unwrap();
    let bytes = archive("@example/notes", "4.5.6");
    let candidate = release(&digest(&bytes));
    let archives = BTreeMap::from([("npm".into(), bytes)]);
    directory
        .submit_package("author", &candidate, &archives, 101)
        .unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection.execute(
        "INSERT INTO submissions(identity,publisher,body,digest,state) VALUES(?1,?2,'legacy','legacy','published')",
        rusqlite::params!["example.notes@1.2.3", "example-publisher"],
    ).unwrap();
    let key = SigningKey::from_bytes(&[23; 32]);
    assert!(
        directory
            .publish("reviewer", 0, 110, 210, "key", &key)
            .is_err()
    );
    assert!(
        directory
            .publish_linked_cargo("reviewer", 0, 110, 210, "key", &key)
            .is_err()
    );
    assert!(
        directory
            .publish_package("reviewer", 0, 110, 210, "key", &key)
            .is_err()
    );
}
