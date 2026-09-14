//! Generates test-only vectors; this deterministic key must never be trusted in production.
use ed25519_dalek::SigningKey;
use lenso_marketplace_catalog::*;
use std::{collections::BTreeMap, fs};
fn main() -> anyhow::Result<()> {
    let output = std::env::args()
        .nth(1)
        .expect("usage: conformance OUTPUT.json");
    let key = SigningKey::from_bytes(&[17; 32]);
    let trust = Trust {
        catalog_id: "conformance-only".into(),
        keys: BTreeMap::from([("test-key".into(), key.verifying_key())]),
    };
    let snapshot = Snapshot::new("conformance-only".into(), 1, 100, 200, vec![]);
    let signed = sign(&snapshot, "test-key", &key)?;
    verify(&signed, &trust, None, 150)?;
    let vector = serde_json::json!({ "public_key_hex": hex::encode(key.verifying_key().to_bytes()), "envelope": serde_json::from_slice::<serde_json::Value>(&signed)?, "expected_payload": snapshot });
    fs::write(output, serde_json::to_vec_pretty(&vector)?)?;
    Ok(())
}
