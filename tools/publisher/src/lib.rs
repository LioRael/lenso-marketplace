//! Author-owned preparation; it requires neither operator configuration nor keys.
use anyhow::{Context, Result, ensure};
use lenso_app_authoring::bundle_archive::{PluginArchiveIdentity, VerifiedPluginArchive};
use lenso_plugin_catalog::{Artifact, Availability, Presentation, Release, digest};
use serde::Deserialize;
use std::{fs, io::Read, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    publisher_id: String,
    title: String,
    summary: String,
    #[serde(default)]
    description: String,
    presentation: Option<Presentation>,
    source_url: String,
    source_revision: String,
    license: String,
    artifact_url: String,
}

pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .with_context(|| format!("read {}", path.display()))?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "{} exceeds {limit} bytes",
        path.display()
    );
    Ok(bytes)
}

fn archive(bytes: &[u8]) -> Result<VerifiedPluginArchive> {
    VerifiedPluginArchive::read(
        bytes,
        &PluginArchiveIdentity {
            size: bytes.len() as u64,
            sha256: digest(bytes),
        },
    )
    .context("verify Plugin archive")
}

pub fn prepare(bundle: &Path, metadata: &Path, output: &Path) -> Result<Release> {
    let metadata: Metadata = serde_json::from_slice(&read_bounded(metadata, 64 * 1024)?)
        .context("read release metadata")?;
    let bytes = read_bounded(bundle, 256 * 1024 * 1024)?;
    let verified = archive(&bytes)?;
    let identity = verified.bundle();
    let release = Release {
        plugin_id: identity.plugin_id.clone(),
        version: identity.release_version.clone(),
        artifact: Artifact {
            url: metadata.artifact_url,
            digest: digest(&bytes),
            size: bytes.len() as u64,
            manifest_digest: identity.manifest_digest.clone(),
        },
        publisher_id: metadata.publisher_id,
        title: metadata.title,
        summary: metadata.summary,
        description: metadata.description,
        presentation: metadata.presentation,
        source_url: metadata.source_url,
        source_revision: metadata.source_revision,
        license: metadata.license,
        availability: Availability::Listed,
    };
    release.validate()?;
    // Never overwrite a previous candidate. Failed writes leave evidence for inspection.
    fs::create_dir(output).context("create a new submission directory")?;
    fs::write(output.join("plugin.lenso-plugin"), bytes)?;
    fs::write(
        output.join("release.json"),
        serde_json::to_vec_pretty(&release)?,
    )?;
    Ok(release)
}

pub fn check(directory: &Path) -> Result<(Release, VerifiedPluginArchive)> {
    let release: Release =
        serde_json::from_slice(&read_bounded(&directory.join("release.json"), 64 * 1024)?)?;
    let bytes = read_bounded(&directory.join("plugin.lenso-plugin"), 256 * 1024 * 1024)?;
    release.verify_archive_bytes(&bytes)?;
    ensure!(
        release.availability == Availability::Listed,
        "submission must be a listed candidate"
    );
    let verified = archive(&bytes)?;
    release.verify_bundle_directory(&verified.directory())?;
    Ok((release, verified))
}
