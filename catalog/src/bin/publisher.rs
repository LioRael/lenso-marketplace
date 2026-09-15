//! Local operator boundary: configuration, database and stdin signer access are
//! controlled by the OS or an approved CI environment, never a public request.
use anyhow::{Context, Result, ensure};
use ed25519_dalek::SigningKey;
use lenso_marketplace_catalog::{
    digest,
    directory::{Directory, PublishedDirectory},
};
use serde::Deserialize;
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

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ensure!(
        args.len() >= 2,
        "usage: lenso-marketplace-publisher CONFIG initialize|export|verify|publish [ACTOR EXPECTED_REVISION VALIDITY_SECONDS]"
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
        "verify" => {
            ensure!(args.len() == 2, "verify accepts no extra arguments");
            let mut envelope = Vec::new();
            io::stdin()
                .lock()
                .take((lenso_marketplace_catalog::MAX_ENVELOPE_BYTES + 1) as u64)
                .read_to_end(&mut envelope)?;
            ensure!(
                envelope.len() <= lenso_marketplace_catalog::MAX_ENVELOPE_BYTES,
                "envelope exceeds limit"
            );
            let trust = lenso_marketplace_catalog::Trust {
                catalog_id: config.catalog_id,
                keys: std::collections::BTreeMap::from([(
                    config.key_id,
                    ed25519_dalek::VerifyingKey::from_bytes(&public_key.as_slice().try_into()?)?,
                )]),
            };
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let verified = lenso_marketplace_catalog::verify(&envelope, &trust, None, now)?;
            let snapshot = verified.snapshot();
            let receipt = serde_json::json!({"catalog_id": snapshot.catalog_id, "revision": snapshot.revision, "expires_at": snapshot.expires_at});
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
        "export" => {
            ensure!(args.len() == 2, "export accepts no extra arguments");
            let view = PublishedDirectory::open(&config.database, &config.catalog_id)?;
            let envelope = view.latest()?.context("no publication exists")?;
            emit(envelope.as_bytes())?;
        }
        "publish" => {
            ensure!(
                args.len() == 5,
                "publish requires actor, expected revision and validity seconds"
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
            let bytes = directory.publish(
                &args[2],
                expected,
                now,
                now.checked_add(validity).context("expiry overflow")?,
                &config.key_id,
                &key,
            )?;
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
