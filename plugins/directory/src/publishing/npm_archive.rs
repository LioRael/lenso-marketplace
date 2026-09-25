use super::digest;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{collections::BTreeSet, io::Read as _};

pub const MAX_NPM_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4096;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

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
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
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
                && relative
                    .split('/')
                    .all(|component| !matches!(component, "" | "." | "..")),
            "npm archive path is outside its package root"
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
