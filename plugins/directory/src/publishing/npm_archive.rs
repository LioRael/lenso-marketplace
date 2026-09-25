use super::digest;
use anyhow::{Context, Result, ensure};
use flate2::bufread::GzDecoder;
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    io::{Cursor, Read as _},
};

pub const MAX_NPM_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4096;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_TAR_PADDING_BYTES: u64 = 1024 * 1024;
const MAX_TAR_BYTES: u64 =
    MAX_UNPACKED_BYTES + (MAX_ARCHIVE_ENTRIES as u64) * 1024 + MAX_TAR_PADDING_BYTES;
const SOURCE_LOCK: &str = ".lenso-npm-source.json";
const SOURCE_ARCHIVE: &str = ".lenso-npm-archive.tgz";

#[derive(Deserialize)]
struct NpmManifest {
    name: String,
    version: String,
    lenso: NpmPluginMetadata,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpmPluginMetadata {
    plugin_id: String,
    release_version: String,
    runtime: String,
    root_slot: String,
}

pub(super) fn verify_npm_archive(
    bytes: &[u8],
    package: &str,
    version: &str,
    plugin_id: &str,
    release_version: &str,
    integrity: &str,
) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_NPM_ARCHIVE_BYTES,
        "npm archive size exceeds limit"
    );
    ensure!(
        digest(bytes) == integrity,
        "npm archive digest does not match release"
    );
    let mut decoder = GzDecoder::new(Cursor::new(bytes));
    let mut tar_bytes = Vec::new();
    decoder
        .by_ref()
        .take(MAX_TAR_BYTES + 1)
        .read_to_end(&mut tar_bytes)?;
    ensure!(
        u64::try_from(tar_bytes.len())? <= MAX_TAR_BYTES,
        "npm archive exceeds total decompressed tar size limit"
    );
    ensure!(
        decoder.into_inner().position() == bytes.len() as u64,
        "npm archive has trailing compressed data"
    );
    let mut archive = tar::Archive::new(tar_bytes.as_slice());
    let mut manifest = None;
    let mut has_bun_lock = false;
    let mut unpacked = 0u64;
    let mut count = 0usize;
    let mut paths = BTreeSet::new();
    for entry in archive.entries().context("invalid npm archive")? {
        let mut entry = entry.context("invalid npm archive entry")?;
        count += 1;
        ensure!(
            count <= MAX_ARCHIVE_ENTRIES,
            "npm archive has too many entries"
        );
        unpacked = unpacked
            .checked_add(entry.size())
            .context("npm archive size overflow")?;
        ensure!(
            unpacked <= MAX_UNPACKED_BYTES,
            "npm archive uncompressed size exceeds limit"
        );
        let path = std::str::from_utf8(&entry.path_bytes())?.to_owned();
        let relative = path
            .strip_prefix("package/")
            .context("npm archive path is outside its package root")?;
        ensure!(
            !relative.contains('\\')
                && !relative.chars().any(char::is_control)
                && relative
                    .split('/')
                    .all(|component| !matches!(component, "" | "." | ".." | "node_modules"))
                && relative != SOURCE_LOCK
                && relative != SOURCE_ARCHIVE,
            "npm archive has an invalid or reserved path"
        );
        ensure!(
            paths.insert(path.clone()),
            "npm archive contains a duplicate path"
        );
        ensure!(
            entry.header().entry_type().is_file(),
            "npm archive contains a non-file entry"
        );
        if relative == "package.json" {
            ensure!(
                entry.size() <= MAX_MANIFEST_BYTES,
                "npm package manifest exceeds size limit"
            );
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents)?;
            let parsed: NpmManifest =
                serde_json::from_slice(&contents).context("invalid npm package.json")?;
            manifest = Some(parsed);
        } else if relative == "bun.lock" {
            has_bun_lock = true;
        }
    }
    let padding = archive.into_inner();
    ensure!(
        u64::try_from(padding.len())? <= MAX_TAR_PADDING_BYTES
            && padding.iter().all(|byte| *byte == 0),
        "npm archive has nonzero or excessive tar data after end marker"
    );
    let manifest = manifest.context("npm archive is missing package.json")?;
    ensure!(
        manifest.name == package,
        "npm archive package name does not match release"
    );
    ensure!(
        manifest.version == version,
        "npm archive package version does not match release"
    );
    ensure!(
        manifest.lenso.plugin_id == plugin_id
            && manifest.lenso.release_version == release_version
            && manifest.lenso.runtime == "bun"
            && !manifest.lenso.root_slot.trim().is_empty(),
        "npm archive Lenso Bun Plugin identity does not match release"
    );
    ensure!(
        has_bun_lock,
        "npm Bun Plugin archive is missing root bun.lock"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn tar_bytes(extra_path: Option<&str>) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        let manifest = serde_json::to_vec(&serde_json::json!({
            "name":"@example/linked", "version":"1.2.3",
            "lenso":{
                "pluginId":"example.linked", "releaseVersion":"1.2.3",
                "runtime":"bun", "rootSlot":"tools"
            }
        }))
        .unwrap();
        for (path, contents) in [
            ("package/package.json", manifest.as_slice()),
            ("package/bun.lock", b"{}\n".as_slice()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, Cursor::new(contents))
                .unwrap();
        }
        if let Some(path) = extra_path {
            let mut header = tar::Header::new_gnu();
            header.set_size(1);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, Cursor::new(b"x"))
                .unwrap();
        }
        builder.into_inner().unwrap()
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn accepted(bytes: &[u8]) -> bool {
        verify_npm_archive(
            bytes,
            "@example/linked",
            "1.2.3",
            "example.linked",
            "1.2.3",
            &digest(bytes),
        )
        .is_ok()
    }

    #[test]
    fn rejects_archive_tails_and_engine_reserved_paths() {
        let tar = tar_bytes(None);
        let mut archive = gzip(&tar);
        assert!(accepted(&archive));

        archive.extend_from_slice(b"hidden gzip tail");
        assert!(!accepted(&archive));
        let mut nonzero_tar_tail = tar;
        nonzero_tar_tail.push(b'x');
        assert!(!accepted(&gzip(&nonzero_tar_tail)));

        for path in [
            "package/node_modules/hidden.js",
            "package/.lenso-npm-source.json",
            "package/.lenso-npm-archive.tgz",
            "package/bad\nname.js",
        ] {
            assert!(!accepted(&gzip(&tar_bytes(Some(path)))), "{path:?}");
        }
    }
}
