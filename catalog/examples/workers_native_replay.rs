//! Replay the exact Workers signed fixtures through the native persisted cache.
use lenso_marketplace_catalog::{Trust, cache::VerifiedCache};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
fn main() -> anyhow::Result<()> {
    let fixtures = PathBuf::from(std::env::args().nth(1).expect("fixture directory"));
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("cache.sqlite");
    let trust = Trust {
        catalog_id: "workers-g3-proof".into(),
        keys: BTreeMap::from([(
            "test-key".into(),
            ed25519_dalek::SigningKey::from_bytes(&[17; 32]).verifying_key(),
        )]),
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let mut cache = VerifiedCache::open(&path, trust.clone())?;
    let mut records = Vec::new();
    for (name, accepted) in [
        ("first", true),
        ("equivocation", false),
        ("tampered", false),
        ("expired", false),
        ("second", true),
        ("identity-change", false),
        ("first", false),
        ("large", true),
        ("restored", true),
    ] {
        let bytes = std::fs::read(fixtures.join(format!("{name}.json")))?;
        let result = cache.accept(&bytes, now);
        anyhow::ensure!(
            result.is_ok() == accepted,
            "unexpected native fixture outcome: {name}: {result:?}"
        );
        records.push(serde_json::json!({"fixture":name,"accepted":accepted}));
    }
    drop(cache);
    let recreated = VerifiedCache::open(&path, trust)?;
    anyhow::ensure!(
        recreated
            .current(now)?
            .expect("persisted snapshot")
            .snapshot()
            .revision
            == 5,
        "checkpoint did not persist"
    );
    println!(
        "{}",
        serde_json::json!({"passed":true,"records":records,"recreated_revision":5})
    );
    Ok(())
}
