use lenso_marketplace_directory_plugin::publishing::release_content::BaseKind;
use lenso_plugin_catalog::digest;
use std::{fs, path::Path};

fn archive() -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    let bytes = b"# Editable template\n";
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "README.md", &bytes[..])
        .unwrap();
    tar.into_inner().unwrap().finish().unwrap()
}

fn write_draft(directory: &Path, value: serde_json::Value) {
    fs::write(
        directory.join("draft.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}

#[test]
fn prepares_package_attached_content_from_base_without_duplicate_metadata() {
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path();
    let bytes = archive();
    fs::write(source.join("template.tar.gz"), &bytes).unwrap();
    write_draft(
        source,
        serde_json::json!({
            "content": [{
                "id": "starter", "kind": "editable_template",
                "url": "https://example.test/starter.tar.gz", "file": "template.tar.gz"
            }]
        }),
    );
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/d16/package-release.json");
    let output = source.join("prepared");
    let release = lenso_marketplace_publisher::prepare_release_content(
        BaseKind::Package,
        Some(&base),
        &source.join("draft.json"),
        &output,
    )
    .unwrap();
    assert_eq!(release.plugin_id, "example.editor");
    assert_eq!(release.version, "1.0.0");
    assert!(release.metadata.is_none());
    assert_eq!(release.content[0].digest, digest(&bytes));
    assert_eq!(release.content[0].size, bytes.len() as u64);
    assert_eq!(
        release.base_release_identity,
        "sha256:e4d3041c85808055ca7141472e4b04faade1663756819413e4d9f645999d1121"
    );
    assert_eq!(
        lenso_marketplace_publisher::check_release_content(&output).unwrap(),
        release
    );
    fs::write(output.join("content-01.tar.gz"), b"tampered").unwrap();
    assert!(lenso_marketplace_publisher::check_release_content(&output).is_err());
}

#[test]
fn prepares_self_bound_content_only_with_exact_local_markdown() {
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path();
    let bytes = archive();
    fs::write(source.join("template.tar.gz"), &bytes).unwrap();
    fs::write(source.join("start.md"), b"# Get started\n\n").unwrap();
    write_draft(
        source,
        serde_json::json!({
            "plugin_id": "example.editor.source", "version": "1.0.0",
            "metadata": {
                "publisher_id": "publisher", "title": "Source Editor",
                "summary": "Editable source template",
                "source_url": "https://example.test/source",
                "source_revision": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "license": "MIT",
                "documentation": [{
                    "id": "start", "revision": "r1", "language": "en",
                    "topic": "getting-started",
                    "url": "https://example.test/source/start.md",
                    "media_type": "text/markdown", "file": "start.md"
                }]
            },
            "content": [{
                "id": "starter", "kind": "editable_template",
                "url": "https://example.test/starter.tar.gz", "file": "template.tar.gz"
            }]
        }),
    );
    let output = source.join("prepared");
    let release = lenso_marketplace_publisher::prepare_release_content(
        BaseKind::ContentOnly,
        None,
        &source.join("draft.json"),
        &output,
    )
    .unwrap();
    assert_eq!(
        release.base_release_identity,
        release.content_only_identity().unwrap()
    );
    assert_eq!(
        release.metadata.as_ref().unwrap().documentation[0].digest,
        digest(b"# Get started\n\n")
    );
    assert_eq!(
        lenso_marketplace_publisher::check_release_content(&output).unwrap(),
        release
    );
    fs::write(output.join("documentation-01.md"), b"changed").unwrap();
    assert!(lenso_marketplace_publisher::check_release_content(&output).is_err());
}

#[test]
fn rejects_attached_metadata_duplication_and_unsafe_input_paths() {
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path();
    fs::write(source.join("template.tar.gz"), archive()).unwrap();
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/d16/package-release.json");
    write_draft(
        source,
        serde_json::json!({
            "plugin_id": "example.editor", "content": [{
                "id": "starter", "kind": "editable_template",
                "url": "https://example.test/starter.tar.gz", "file": "template.tar.gz"
            }]
        }),
    );
    let output = source.join("duplicate");
    assert!(
        lenso_marketplace_publisher::prepare_release_content(
            BaseKind::Package,
            Some(&base),
            &source.join("draft.json"),
            &output
        )
        .is_err()
    );
    assert!(!output.exists());
    write_draft(
        source,
        serde_json::json!({
            "content": [{
                "id": "starter", "kind": "editable_template",
                "url": "https://example.test/starter.tar.gz", "file": "../template.tar.gz"
            }]
        }),
    );
    assert!(
        lenso_marketplace_publisher::prepare_release_content(
            BaseKind::Package,
            Some(&base),
            &source.join("draft.json"),
            &output
        )
        .is_err()
    );
    assert!(!output.exists());
}
