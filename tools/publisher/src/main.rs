//! Local operator boundary: configuration, database and stdin signer access are
//! controlled by the OS or an approved CI environment, never a public request.
use anyhow::{Context, Result, ensure};
use ed25519_dalek::SigningKey;
use lenso_marketplace_directory_plugin::publishing::MAX_NPM_ARCHIVE_BYTES;
use lenso_marketplace_directory_plugin::publishing::release_content::{self, ReleaseContent};
use lenso_marketplace_directory_plugin::publishing::{Directory, PublishedDirectory};
#[cfg(feature = "package-publication")]
use lenso_plugin_catalog::package::PackageRelease;
use lenso_plugin_catalog::{
    DistributionKind, ReleaseDetails, digest, linked_cargo::LinkedCargoRelease,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::{
    collections::BTreeSet,
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    database: PathBuf,
    catalog_id: String,
    reviewers: BTreeSet<String>,
    key_id: String,
    public_key_hex: String,
}

fn read_npm_archives(pairs: &[String], expected: usize) -> Result<BTreeMap<String, Vec<u8>>> {
    ensure!(
        pairs.len() == expected.checked_mul(2).context("too many npm archives")?,
        "one exact npm archive is required for each npm distribution"
    );
    let mut archives = BTreeMap::new();
    for pair in pairs.chunks_exact(2) {
        let mut bytes = Vec::new();
        fs::File::open(&pair[1])?
            .take((MAX_NPM_ARCHIVE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= MAX_NPM_ARCHIVE_BYTES,
            "npm archive exceeds limit"
        );
        ensure!(
            archives.insert(pair[0].clone(), bytes).is_none(),
            "duplicate npm distribution ID"
        );
    }
    Ok(archives)
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ensure!(
        args.len() >= 2,
        "usage: lenso-marketplace-publisher CONFIG OPERATION [...]; package-only operations require --features package-publication"
    );
    ensure!(
        cfg!(feature = "package-publication")
            || !matches!(
                args[1].as_str(),
                "submit-package"
                    | "inspect-package"
                    | "approve-package"
                    | "publish-package"
                    | "export-package"
                    | "verify-package"
            ),
        "package-only publication is not available in this build; the package-publication feature requires the newer signed package protocol"
    );
    let config: Config = serde_json::from_slice(&fs::read(&args[0])?)?;
    ensure!(
        config.database.is_absolute(),
        "database path must be absolute"
    );
    ensure!(
        !config.catalog_id.is_empty() && !config.key_id.is_empty() && !config.reviewers.is_empty(),
        "catalog, key and reviewers must be configured"
    );
    let public_key = hex::decode(&config.public_key_hex).context("invalid public key hex")?;
    ensure!(public_key.len() == 32, "public key must contain 32 bytes");
    match args[1].as_str() {
        "release-content-base" => {
            ensure!(
                args.len() == 4,
                "release-content-base requires portable|linked_cargo RELEASE_JSON"
            );
            let bytes = fs::read(&args[3])?;
            ensure!(
                bytes.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                "base release exceeds limit"
            );
            let identity = match args[2].as_str() {
                "portable" => {
                    let release: lenso_plugin_catalog::Release = serde_json::from_slice(&bytes)?;
                    release.validate()?;
                    release.immutable_identity()?
                }
                "linked_cargo" => {
                    let release: LinkedCargoRelease = serde_json::from_slice(&bytes)?;
                    release_content::linked_identity(&release)?
                }
                _ => anyhow::bail!("base kind must be portable or linked_cargo"),
            };
            serde_json::to_writer(
                io::stdout().lock(),
                &serde_json::json!({"base_release_identity":identity}),
            )?;
            writeln!(io::stdout().lock())?;
        }
        "claim"
        | "submit"
        | "inspect"
        | "approve"
        | "submit-details"
        | "inspect-details"
        | "approve-details"
        | "submit-details-revision"
        | "inspect-details-revision"
        | "approve-details-revision"
        | "submit-linked-cargo"
        | "inspect-linked-cargo"
        | "approve-linked-cargo"
        | "submit-linked-cargo-docs-revision"
        | "inspect-linked-cargo-docs-revision"
        | "approve-linked-cargo-docs-revision"
        | "submit-package"
        | "inspect-package"
        | "approve-package"
        | "submit-release-content"
        | "inspect-release-content"
        | "approve-release-content" => {
            // These are protected local operator commands, never public actor authentication.
            PublishedDirectory::open(&config.database, &config.catalog_id)?;
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let mut directory =
                Directory::open(&config.database, &config.catalog_id, config.reviewers)?;
            let receipt = match args[1].as_str() {
                "claim" => {
                    ensure!(
                        args.len() == 6,
                        "claim requires REVIEWER NAMESPACE PUBLISHER AUTHOR"
                    );
                    directory.claim_namespace(&args[2], &args[3], &args[4], &args[5], now)?;
                    serde_json::json!({"status":"claimed", "namespace":args[3]})
                }
                "submit" => {
                    ensure!(
                        args.len() == 4,
                        "submit requires AUTHOR SUBMISSION_DIRECTORY"
                    );
                    let (release, verified) =
                        lenso_marketplace_publisher::check(std::path::Path::new(&args[3]))?;
                    let mut bytes = Vec::new();
                    verified.open_archive()?.read_to_end(&mut bytes)?;
                    let id =
                        directory.submit(&args[2], &release, &bytes, &verified.directory(), now)?;
                    let (_, proposal_digest, state) = directory.inspect_submission(&args[2], id)?;
                    serde_json::json!({"submission_id":id, "proposal_digest":proposal_digest, "state":state})
                }
                "inspect" => {
                    ensure!(args.len() == 4, "inspect requires ACTOR SUBMISSION_ID");
                    let (release, proposal_digest, state) =
                        directory.inspect_submission(&args[2], args[3].parse()?)?;
                    serde_json::json!({"release":release,"proposal_digest":proposal_digest,"state":state})
                }
                "approve" => {
                    ensure!(
                        args.len() == 6,
                        "approve requires REVIEWER SUBMISSION_ID EXPECTED_DIGEST POLICY"
                    );
                    directory.approve(&args[2], args[3].parse()?, &args[4], &args[5], now)?;
                    serde_json::json!({"status":"approved","submission_id":args[3]})
                }
                "submit-details" => {
                    ensure!(
                        args.len() >= 4 && (args.len() - 4) % 2 == 0,
                        "submit-details requires AUTHOR DETAILS_JSON [ID ARCHIVE...]"
                    );
                    let bytes = fs::read(&args[3])?;
                    ensure!(
                        bytes.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                        "release details exceed limit"
                    );
                    let details: ReleaseDetails = serde_json::from_slice(&bytes)?;
                    details.validate()?;
                    let npm_count = details
                        .distributions
                        .iter()
                        .filter(|distribution| distribution.kind == DistributionKind::NpmPackage)
                        .count();
                    let archives = read_npm_archives(&args[4..], npm_count)?;
                    let id = directory
                        .submit_details_with_archives(&args[2], &details, &archives, now)?;
                    let (_, proposal_digest, state) =
                        directory.inspect_details_submission(&args[2], id)?;
                    serde_json::json!({"submission_id":id,"proposal_digest":proposal_digest,"state":state})
                }
                "submit-details-revision" => {
                    ensure!(
                        args.len() == 4,
                        "submit-details-revision requires AUTHOR DETAILS_JSON"
                    );
                    let bytes = fs::read(&args[3])?;
                    ensure!(
                        bytes.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                        "release details exceed limit"
                    );
                    let details: ReleaseDetails = serde_json::from_slice(&bytes)?;
                    let id = directory.submit_details_revision(&args[2], &details, now)?;
                    let (_, proposal_digest, state) =
                        directory.inspect_details_revision(&args[2], id)?;
                    serde_json::json!({"revision_id":id,"proposal_digest":proposal_digest,"state":state})
                }
                "inspect-details" => {
                    ensure!(
                        args.len() == 4,
                        "inspect-details requires ACTOR SUBMISSION_ID"
                    );
                    let (details, proposal_digest, state) =
                        directory.inspect_details_submission(&args[2], args[3].parse()?)?;
                    serde_json::json!({"release_details":details,"proposal_digest":proposal_digest,"state":state})
                }
                "inspect-details-revision" => {
                    ensure!(
                        args.len() == 4,
                        "inspect-details-revision requires ACTOR REVISION_ID"
                    );
                    let (details, proposal_digest, state) =
                        directory.inspect_details_revision(&args[2], args[3].parse()?)?;
                    serde_json::json!({"release_details":details,"proposal_digest":proposal_digest,"state":state})
                }
                "approve-details" => {
                    ensure!(
                        args.len() == 6,
                        "approve-details requires REVIEWER SUBMISSION_ID EXPECTED_DIGEST POLICY"
                    );
                    directory.approve_details(
                        &args[2],
                        args[3].parse()?,
                        &args[4],
                        &args[5],
                        now,
                    )?;
                    serde_json::json!({"status":"approved","submission_id":args[3]})
                }
                "approve-details-revision" => {
                    ensure!(
                        args.len() == 6,
                        "approve-details-revision requires REVIEWER REVISION_ID EXPECTED_DIGEST POLICY"
                    );
                    directory.approve_details_revision(
                        &args[2],
                        args[3].parse()?,
                        &args[4],
                        &args[5],
                        now,
                    )?;
                    serde_json::json!({"status":"approved","revision_id":args[3]})
                }
                "submit-linked-cargo" => {
                    ensure!(
                        args.len() == 5,
                        "submit-linked-cargo requires AUTHOR RELEASE_JSON CRATE_ARCHIVE"
                    );
                    let release_bytes = fs::read(&args[3])?;
                    ensure!(
                        release_bytes.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                        "linked Cargo release exceeds limit"
                    );
                    let release: LinkedCargoRelease = serde_json::from_slice(&release_bytes)?;
                    ensure!(
                        fs::metadata(&args[4])?.len() <= 32 * 1024 * 1024,
                        "crate archive exceeds limit"
                    );
                    let archive = fs::read(&args[4])?;
                    let id = directory.submit_linked_cargo(&args[2], &release, &archive, now)?;
                    let (_, proposal_digest, state) =
                        directory.inspect_linked_cargo_submission(&args[2], id)?;
                    serde_json::json!({"submission_id":id,"proposal_digest":proposal_digest,"state":state})
                }
                #[cfg(feature = "package-publication")]
                "submit-package" => {
                    ensure!(
                        args.len() >= 6 && (args.len() - 4) % 2 == 0,
                        "submit-package requires AUTHOR RELEASE_JSON ID ARCHIVE [ID ARCHIVE...]"
                    );
                    let release_bytes = fs::read(&args[3])?;
                    ensure!(
                        release_bytes.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                        "package release exceeds limit"
                    );
                    let release: PackageRelease = serde_json::from_slice(&release_bytes)?;
                    release.validate()?;
                    let archives = read_npm_archives(&args[4..], release.distributions.len())?;
                    let id = directory.submit_package(&args[2], &release, &archives, now)?;
                    let (_, proposal_digest, state) =
                        directory.inspect_package_submission(&args[2], id)?;
                    serde_json::json!({"submission_id":id,"proposal_digest":proposal_digest,"state":state})
                }
                #[cfg(feature = "package-publication")]
                "inspect-package" => {
                    ensure!(
                        args.len() == 4,
                        "inspect-package requires ACTOR SUBMISSION_ID"
                    );
                    let (release, proposal_digest, state) =
                        directory.inspect_package_submission(&args[2], args[3].parse()?)?;
                    serde_json::json!({"release":release,"proposal_digest":proposal_digest,"state":state})
                }
                #[cfg(feature = "package-publication")]
                "approve-package" => {
                    ensure!(
                        args.len() == 6,
                        "approve-package requires REVIEWER SUBMISSION_ID EXPECTED_DIGEST POLICY"
                    );
                    directory.approve_package(
                        &args[2],
                        args[3].parse()?,
                        &args[4],
                        &args[5],
                        now,
                    )?;
                    serde_json::json!({"status":"approved","submission_id":args[3]})
                }
                "inspect-linked-cargo" => {
                    ensure!(
                        args.len() == 4,
                        "inspect-linked-cargo requires ACTOR SUBMISSION_ID"
                    );
                    let (release, proposal_digest, state) =
                        directory.inspect_linked_cargo_submission(&args[2], args[3].parse()?)?;
                    serde_json::json!({"release":release,"proposal_digest":proposal_digest,"state":state})
                }
                "approve-linked-cargo" => {
                    ensure!(
                        args.len() == 6,
                        "approve-linked-cargo requires REVIEWER SUBMISSION_ID EXPECTED_DIGEST POLICY"
                    );
                    directory.approve_linked_cargo(
                        &args[2],
                        args[3].parse()?,
                        &args[4],
                        &args[5],
                        now,
                    )?;
                    serde_json::json!({"status":"approved","submission_id":args[3]})
                }
                "submit-linked-cargo-docs-revision" => {
                    ensure!(
                        args.len() == 4,
                        "submit-linked-cargo-docs-revision requires AUTHOR RELEASE_JSON"
                    );
                    let bytes = fs::read(&args[3])?;
                    ensure!(
                        bytes.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                        "linked Cargo release exceeds limit"
                    );
                    let release: LinkedCargoRelease = serde_json::from_slice(&bytes)?;
                    let id =
                        directory.submit_linked_cargo_document_revision(&args[2], &release, now)?;
                    let (_, proposal_digest, state) =
                        directory.inspect_linked_cargo_document_revision(&args[2], id)?;
                    serde_json::json!({"revision_id":id,"proposal_digest":proposal_digest,"state":state})
                }
                "inspect-linked-cargo-docs-revision" => {
                    ensure!(
                        args.len() == 4,
                        "inspect-linked-cargo-docs-revision requires ACTOR REVISION_ID"
                    );
                    let (release, proposal_digest, state) = directory
                        .inspect_linked_cargo_document_revision(&args[2], args[3].parse()?)?;
                    serde_json::json!({"release":release,"proposal_digest":proposal_digest,"state":state})
                }
                "approve-linked-cargo-docs-revision" => {
                    ensure!(
                        args.len() == 6,
                        "approve-linked-cargo-docs-revision requires REVIEWER REVISION_ID EXPECTED_DIGEST POLICY"
                    );
                    directory.approve_linked_cargo_document_revision(
                        &args[2],
                        args[3].parse()?,
                        &args[4],
                        &args[5],
                        now,
                    )?;
                    serde_json::json!({"status":"approved","revision_id":args[3]})
                }
                "submit-release-content" => {
                    ensure!(
                        args.len() >= 5,
                        "submit-release-content requires AUTHOR RELEASE_JSON ARCHIVE..."
                    );
                    let bytes = fs::read(&args[3])?;
                    ensure!(
                        bytes.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                        "release content submission exceeds limit"
                    );
                    let release: ReleaseContent = serde_json::from_slice(&bytes)?;
                    ensure!(
                        args.len() == 4 + release.content.len(),
                        "one archive is required per release content entry, in content order"
                    );
                    let mut archives = Vec::with_capacity(release.content.len());
                    for path in &args[4..] {
                        ensure!(
                            fs::metadata(path)?.len() <= 16 * 1024 * 1024,
                            "release content archive exceeds limit"
                        );
                        archives.push(fs::read(path)?);
                    }
                    let views: Vec<&[u8]> = archives.iter().map(Vec::as_slice).collect();
                    let id = directory.submit_release_content(&args[2], &release, &views, now)?;
                    let (_, proposal_digest, state) =
                        directory.inspect_release_content_submission(&args[2], id)?;
                    serde_json::json!({"submission_id":id,"proposal_digest":proposal_digest,"state":state})
                }
                "inspect-release-content" => {
                    ensure!(
                        args.len() == 4,
                        "inspect-release-content requires ACTOR SUBMISSION_ID"
                    );
                    let (release, proposal_digest, state) =
                        directory.inspect_release_content_submission(&args[2], args[3].parse()?)?;
                    serde_json::json!({"release_content":release,"proposal_digest":proposal_digest,"state":state})
                }
                "approve-release-content" => {
                    ensure!(
                        args.len() == 6,
                        "approve-release-content requires REVIEWER SUBMISSION_ID EXPECTED_DIGEST POLICY"
                    );
                    directory.approve_release_content(
                        &args[2],
                        args[3].parse()?,
                        &args[4],
                        &args[5],
                        now,
                    )?;
                    serde_json::json!({"status":"approved","submission_id":args[3]})
                }
                _ => unreachable!(),
            };
            serde_json::to_writer(io::stdout().lock(), &receipt)?;
            writeln!(io::stdout().lock())?;
        }
        "verify" => {
            ensure!(args.len() == 2, "verify accepts no extra arguments");
            let mut envelope = Vec::new();
            io::stdin()
                .lock()
                .take((lenso_plugin_catalog::MAX_ENVELOPE_BYTES + 1) as u64)
                .read_to_end(&mut envelope)?;
            ensure!(
                envelope.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                "envelope exceeds limit"
            );
            let trust = lenso_plugin_catalog::Trust {
                catalog_id: config.catalog_id,
                keys: std::collections::BTreeMap::from([(
                    config.key_id,
                    ed25519_dalek::VerifyingKey::from_bytes(&public_key.as_slice().try_into()?)?,
                )]),
            };
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let verified = lenso_plugin_catalog::verify(&envelope, &trust, None, now)?;
            let snapshot = verified.snapshot();
            let receipt = serde_json::json!({"catalog_id": snapshot.catalog_id, "revision": snapshot.revision, "expires_at": snapshot.expires_at});
            serde_json::to_writer(io::stdout().lock(), &receipt)?;
        }
        "verify-details" => {
            ensure!(args.len() == 2, "verify-details accepts no extra arguments");
            let mut envelope = Vec::new();
            io::stdin()
                .lock()
                .take((lenso_plugin_catalog::MAX_ENVELOPE_BYTES + 1) as u64)
                .read_to_end(&mut envelope)?;
            ensure!(
                envelope.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                "envelope exceeds limit"
            );
            let trust = lenso_plugin_catalog::Trust {
                catalog_id: config.catalog_id,
                keys: std::collections::BTreeMap::from([(
                    config.key_id,
                    ed25519_dalek::VerifyingKey::from_bytes(&public_key.as_slice().try_into()?)?,
                )]),
            };
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let verified =
                lenso_plugin_catalog::verify_release_details(&envelope, &trust, None, now)?;
            let snapshot = verified.snapshot();
            let documents: Vec<_> = snapshot
                .releases
                .iter()
                .flat_map(|release| &release.documentation)
                .collect();
            let receipt = serde_json::json!({"catalog_id":snapshot.catalog_id,"revision":snapshot.revision,"expires_at":snapshot.expires_at,"documents":documents});
            serde_json::to_writer(io::stdout().lock(), &receipt)?;
        }
        "verify-linked-cargo" => {
            ensure!(
                args.len() == 2,
                "verify-linked-cargo accepts no extra arguments"
            );
            let mut envelope = Vec::new();
            io::stdin()
                .lock()
                .take((lenso_plugin_catalog::MAX_ENVELOPE_BYTES + 1) as u64)
                .read_to_end(&mut envelope)?;
            ensure!(
                envelope.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                "envelope exceeds limit"
            );
            let trust = lenso_plugin_catalog::Trust {
                catalog_id: config.catalog_id,
                keys: std::collections::BTreeMap::from([(
                    config.key_id,
                    ed25519_dalek::VerifyingKey::from_bytes(&public_key.as_slice().try_into()?)?,
                )]),
            };
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let verified =
                lenso_plugin_catalog::linked_cargo::verify(&envelope, &trust, None, now)?;
            let snapshot = verified.snapshot();
            let documents: Vec<_> = snapshot
                .releases
                .iter()
                .flat_map(|release| &release.documentation)
                .collect();
            let receipt = serde_json::json!({"catalog_id":snapshot.catalog_id,"revision":snapshot.revision,"expires_at":snapshot.expires_at,"documents":documents});
            serde_json::to_writer(io::stdout().lock(), &receipt)?;
        }
        #[cfg(feature = "package-publication")]
        "verify-package" => {
            ensure!(args.len() == 2, "verify-package accepts no extra arguments");
            let mut envelope = Vec::new();
            io::stdin()
                .lock()
                .take((lenso_plugin_catalog::MAX_ENVELOPE_BYTES + 1) as u64)
                .read_to_end(&mut envelope)?;
            ensure!(
                envelope.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                "envelope exceeds limit"
            );
            let trust = lenso_plugin_catalog::Trust {
                catalog_id: config.catalog_id,
                keys: std::collections::BTreeMap::from([(
                    config.key_id,
                    ed25519_dalek::VerifyingKey::from_bytes(&public_key.as_slice().try_into()?)?,
                )]),
            };
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let verified = lenso_plugin_catalog::package::verify(&envelope, &trust, None, now)?;
            let snapshot = verified.snapshot();
            let documents: Vec<_> = snapshot
                .releases
                .iter()
                .flat_map(|release| &release.documentation)
                .collect();
            let receipt = serde_json::json!({"catalog_id":snapshot.catalog_id,"revision":snapshot.revision,"expires_at":snapshot.expires_at,"documents":documents});
            serde_json::to_writer(io::stdout().lock(), &receipt)?;
        }
        "verify-release-content" => {
            ensure!(
                args.len() == 2,
                "verify-release-content accepts no extra arguments"
            );
            let mut envelope = Vec::new();
            io::stdin()
                .lock()
                .take((lenso_plugin_catalog::MAX_ENVELOPE_BYTES + 1) as u64)
                .read_to_end(&mut envelope)?;
            ensure!(
                envelope.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
                "envelope exceeds limit"
            );
            let trust = lenso_plugin_catalog::Trust {
                catalog_id: config.catalog_id,
                keys: std::collections::BTreeMap::from([(
                    config.key_id,
                    ed25519_dalek::VerifyingKey::from_bytes(&public_key.as_slice().try_into()?)?,
                )]),
            };
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let snapshot = release_content::verify(&envelope, &trust, now)?;
            let receipt = serde_json::json!({"catalog_id":snapshot.catalog_id,"revision":snapshot.revision,"expires_at":snapshot.expires_at});
            serde_json::to_writer(io::stdout().lock(), &receipt)?;
        }
        "initialize" => {
            ensure!(args.len() == 2, "initialize accepts no extra arguments");
            // create_new prevents accidental reuse; failed initialization leaves
            // the owned file for inspection instead of deleting uncertain state.
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&config.database)?;
            Directory::open(&config.database, &config.catalog_id, config.reviewers)?;
            writeln!(io::stdout().lock(), "{{\"initialized\":true}}")?;
        }
        "backup" => {
            ensure!(
                args.len() == 3,
                "backup requires a new absolute destination path"
            );
            let view = PublishedDirectory::open(&config.database, &config.catalog_id)?;
            let destination = PathBuf::from(&args[2]);
            view.backup(&destination)?;
            let copied = PublishedDirectory::open(&destination, &config.catalog_id)?;
            let latest = copied.latest()?;
            let linked_cargo = copied.latest_linked_cargo()?;
            let package = copied.latest_package()?;
            let release_content = copied.latest_release_content()?;
            let receipt = serde_json::json!({
                "backup": destination,
                "catalog_id": config.catalog_id,
                "publication_digest": latest.as_ref().map(|bytes| digest(bytes.as_bytes())),
                "linked_cargo_publication_digest": linked_cargo.as_ref().map(|bytes| digest(bytes.as_bytes())),
                "package_publication_digest": package.as_ref().map(|bytes| digest(bytes.as_bytes())),
                "release_content_publication_digest": release_content.as_ref().map(|bytes| digest(bytes.as_bytes()))
            });
            serde_json::to_writer(io::stdout().lock(), &receipt)?;
            writeln!(io::stdout().lock())?;
        }
        "export" => {
            ensure!(args.len() == 2, "export accepts no extra arguments");
            let view = PublishedDirectory::open(&config.database, &config.catalog_id)?;
            let envelope = view.latest()?.context("no publication exists")?;
            emit(envelope.as_bytes())?;
        }
        "export-details" => {
            ensure!(args.len() == 2, "export-details accepts no extra arguments");
            let view = PublishedDirectory::open(&config.database, &config.catalog_id)?;
            let envelope = view
                .latest_details()?
                .context("no release details publication exists")?;
            emit(envelope.as_bytes())?;
        }
        "export-linked-cargo" => {
            ensure!(
                args.len() == 2,
                "export-linked-cargo accepts no extra arguments"
            );
            let view = PublishedDirectory::open(&config.database, &config.catalog_id)?;
            let envelope = view
                .latest_linked_cargo()?
                .context("no linked Cargo publication exists")?;
            emit(envelope.as_bytes())?;
        }
        #[cfg(feature = "package-publication")]
        "export-package" => {
            ensure!(args.len() == 2, "export-package accepts no extra arguments");
            let view = PublishedDirectory::open(&config.database, &config.catalog_id)?;
            let envelope = view
                .latest_package()?
                .context("no package publication exists")?;
            emit(envelope.as_bytes())?;
        }
        "export-release-content" => {
            ensure!(
                args.len() == 2,
                "export-release-content accepts no extra arguments"
            );
            let view = PublishedDirectory::open(&config.database, &config.catalog_id)?;
            let envelope = view
                .latest_release_content()?
                .context("no release content publication exists")?;
            emit(envelope.as_bytes())?;
        }
        "publish"
        | "publish-details"
        | "publish-linked-cargo"
        | "publish-package"
        | "publish-release-content" => {
            ensure!(
                args.len() == 5,
                "publish operation requires actor, expected revision and validity seconds"
            );
            ensure!(
                config.reviewers.contains(&args[2]),
                "reviewer authorization required"
            );
            let expected: u64 = args[3].parse()?;
            let validity: u64 = args[4].parse()?;
            ensure!(
                (1..=604800).contains(&validity),
                "validity must be 1..604800 seconds"
            );
            // Opening a public projection first prevents publishing from silently
            // initializing a missing database or selecting another catalog.
            PublishedDirectory::open(&config.database, &config.catalog_id)?;
            let mut secret = Vec::new();
            io::stdin().lock().take(33).read_to_end(&mut secret)?;
            ensure!(
                secret.len() == 32,
                "stdin must supply exactly 32 signing-key bytes"
            );
            let seed: [u8; 32] = secret.as_slice().try_into()?;
            let key = SigningKey::from_bytes(&seed);
            ensure!(
                key.verifying_key().as_bytes().as_slice() == public_key,
                "signer does not match configured public trust"
            );
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let mut directory =
                Directory::open(&config.database, &config.catalog_id, config.reviewers)?;
            let expires_at = now.checked_add(validity).context("expiry overflow")?;
            let bytes = if args[1] == "publish-release-content" {
                directory.publish_release_content(
                    &args[2],
                    expected,
                    now,
                    expires_at,
                    &config.key_id,
                    &key,
                )?
            } else if args[1] == "publish-details" {
                directory.publish_details(
                    &args[2],
                    expected,
                    now,
                    expires_at,
                    &config.key_id,
                    &key,
                )?
            } else if args[1] == "publish-linked-cargo" {
                directory.publish_linked_cargo(
                    &args[2],
                    expected,
                    now,
                    expires_at,
                    &config.key_id,
                    &key,
                )?
            } else if args[1] == "publish-package" {
                #[cfg(feature = "package-publication")]
                {
                    directory.publish_package(
                        &args[2],
                        expected,
                        now,
                        expires_at,
                        &config.key_id,
                        &key,
                    )?
                }
                #[cfg(not(feature = "package-publication"))]
                {
                    anyhow::bail!("package-only publication is not available in this build")
                }
            } else {
                directory.publish(&args[2], expected, now, expires_at, &config.key_id, &key)?
            };
            // A broken pipe does not undo the committed publication. Export is
            // the recovery operation; blindly retrying publish must lose its CAS.
            emit(&bytes)?;
        }
        _ => anyhow::bail!("unknown operation"),
    }
    Ok(())
}

fn emit(envelope: &[u8]) -> Result<()> {
    let receipt =
        serde_json::json!({"envelope": std::str::from_utf8(envelope)?, "digest": digest(envelope)});
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, &receipt)?;
    writeln!(stdout)?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
