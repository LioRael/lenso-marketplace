use ed25519_dalek::SigningKey;
use lenso_marketplace_catalog::{Snapshot, Trust, cache::VerifiedCache, sign};
use std::collections::BTreeMap;

#[test]
fn restart_preserves_checkpoint_and_failed_updates_preserve_current_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("verified.sqlite3");
    let key = SigningKey::from_bytes(&[17; 32]);
    let trust = Trust {
        catalog_id: "cache-test".into(),
        keys: BTreeMap::from([("test-key".into(), key.verifying_key())]),
    };
    let envelope = |revision| {
        sign(
            &Snapshot::new("cache-test".into(), revision, 100, 200, vec![]),
            "test-key",
            &key,
        )
        .unwrap()
    };
    let mut cache = VerifiedCache::open(&path, trust.clone()).unwrap();
    assert!(cache.current(150).unwrap().is_none());
    cache.accept(&envelope(2), 150).unwrap();
    drop(cache);
    let mut cache = VerifiedCache::open(&path, trust).unwrap();
    assert_eq!(cache.current(150).unwrap().unwrap().snapshot().revision, 2);
    assert!(cache.accept(&envelope(1), 150).is_err());
    assert!(cache.accept(b"invalid signature envelope", 150).is_err());
    assert_eq!(cache.current(150).unwrap().unwrap().snapshot().revision, 2);
    assert!(cache.current(201).is_err());
    assert!(cache.accept(&envelope(3), 201).is_err());
    assert_eq!(cache.current(150).unwrap().unwrap().snapshot().revision, 2);
    drop(cache);
    let changed_trust = Trust {
        catalog_id: "cache-test".into(),
        keys: BTreeMap::from([(
            "test-key".into(),
            SigningKey::from_bytes(&[18; 32]).verifying_key(),
        )]),
    };
    assert!(
        VerifiedCache::open(&path, changed_trust)
            .unwrap()
            .current(150)
            .is_err()
    );
}
