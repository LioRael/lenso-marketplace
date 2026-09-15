//! Fixed test-key fixtures for the private real-D1 CAS proof; no production keys.
use anyhow::{Context, Result, ensure};
use lenso_marketplace_catalog::{Trust, digest, persistence::AcceptedEnvelope, sign, verify};
use std::{collections::BTreeMap, io::Write, path::Path};

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

fn main() -> Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        arguments.len() == 2,
        "usage: workers_race_fixtures INPUT_DIR OUTPUT_DIR"
    );
    let input = Path::new(&arguments[0]);
    let output = Path::new(&arguments[1]);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
    let trust = Trust {
        catalog_id: "workers-g3-proof".into(),
        keys: BTreeMap::from([("test-key".into(), key.verifying_key())]),
    };
    // The full smoke accepted the large snapshot before omitting its releases.
    // Carry that history forward; first + restored alone would lose 160 identities.
    let mut checkpoint = None;
    let mut restored = None;
    for (name, revision) in [("first", 1), ("large", 4), ("restored", 5)] {
        let envelope = std::fs::read(input.join(format!("{name}.json")))?;
        let verified = verify(&envelope, &trust, checkpoint.as_ref(), now)
            .with_context(|| format!("verify {name} fixture"))?;
        ensure!(
            verified.snapshot().revision == revision,
            "unexpected {name} revision"
        );
        checkpoint = Some(verified.checkpoint().clone());
        if name == "restored" {
            restored = Some((envelope, verified));
        }
    }
    let (envelope, base) = restored.context("restored fixture missing")?;
    ensure!(
        base.snapshot().releases.is_empty(),
        "restored fixture must omit releases"
    );
    ensure!(
        base.checkpoint().release_identities.len() == 161,
        "expected complete G3 history"
    );
    let expected = AcceptedEnvelope {
        token: digest(&envelope),
        checkpoint: base.checkpoint().clone(),
        envelope,
    };
    let candidate = |revision| -> Result<AcceptedEnvelope> {
        let mut snapshot = base.snapshot().clone();
        snapshot.revision = revision;
        snapshot.issued_at = now;
        let envelope = sign(&snapshot, "test-key", &key)?;
        let verified = verify(&envelope, &trust, Some(base.checkpoint()), now)?;
        Ok(AcceptedEnvelope {
            token: digest(&envelope),
            checkpoint: verified.checkpoint().clone(),
            envelope,
        })
    };
    let candidates = [candidate(6)?, candidate(7)?];
    let restore = candidate(8)?;
    // Revision 8 can safely follow either possible race winner.
    for winner in &candidates {
        verify(&restore.envelope, &trust, Some(&winner.checkpoint), now)?;
    }
    let mut fixture = serde_json::to_vec_pretty(&serde_json::json!({
        "version": 1,
        "catalog": trust.catalog_id,
        "verified_at": now,
        "valid_until": base.snapshot().expires_at,
        "expected": expected,
        "candidates": candidates,
    }))?;
    fixture.push(b'\n');
    std::fs::create_dir_all(output)?;
    write_new(&output.join("race.json"), &fixture)?;
    write_new(&output.join("restored-8.json"), &restore.envelope)?;
    println!(
        "{}",
        serde_json::json!({
            "fixture": output.join("race.json"),
            "fixture_sha256": digest(&fixture).trim_start_matches("sha256:"),
            "expected_revision": 5,
            "candidate_revisions": [6, 7],
            "historical_identities": 161,
            "expires_at": base.snapshot().expires_at,
            "restore_envelope": output.join("restored-8.json"),
        })
    );
    Ok(())
}
