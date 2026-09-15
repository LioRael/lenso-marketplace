//! These vectors prevent event-local caches from losing persistent rollback
//! fences, accepting an equivocation after a race, or serving expired fallback.
use futures::{executor::block_on, future::LocalBoxFuture};
use lenso_marketplace_catalog::{
    Snapshot, Trust,
    persistence::{AcceptedEnvelope, EventCache, SnapshotStorage},
    sign,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

#[derive(Debug, Default)]
struct Store {
    value: RefCell<Option<AcceptedEnvelope>>,
    race: RefCell<Option<AcceptedEnvelope>>,
    fail: Cell<bool>,
}
impl SnapshotStorage for Store {
    fn published<'a>(&'a self, _: &'a str) -> LocalBoxFuture<'a, anyhow::Result<Option<String>>> {
        Box::pin(async { Ok(None) })
    }
    fn accepted<'a>(
        &'a self,
        _: &'a str,
    ) -> LocalBoxFuture<'a, anyhow::Result<Option<AcceptedEnvelope>>> {
        Box::pin(async {
            anyhow::ensure!(!self.fail.get(), "storage unavailable");
            Ok(self.value.borrow().clone())
        })
    }
    fn compare_exchange<'a>(
        &'a self,
        _: &'a str,
        expected: Option<&'a str>,
        value: &'a AcceptedEnvelope,
    ) -> LocalBoxFuture<'a, anyhow::Result<bool>> {
        Box::pin(async move {
            anyhow::ensure!(!self.fail.get(), "storage unavailable");
            if let Some(winner) = self.race.take() {
                self.value.replace(Some(winner));
            }
            let matches = self.value.borrow().as_ref().map(|v| v.token.as_str()) == expected;
            if matches {
                self.value.replace(Some(value.clone()));
            }
            Ok(matches)
        })
    }
}
fn setup() -> (Rc<Store>, EventCache, Trust, ed25519_dalek::SigningKey) {
    let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
    let trust = Trust {
        catalog_id: "test".into(),
        keys: BTreeMap::from([("key".into(), key.verifying_key())]),
    };
    let store = Rc::new(Store::default());
    (
        store.clone(),
        EventCache::new(store, trust.clone()),
        trust,
        key,
    )
}
fn envelope(key: &ed25519_dalek::SigningKey, revision: u64, issued: u64) -> Vec<u8> {
    sign(
        &Snapshot::new("test".into(), revision, issued, 200, vec![]),
        "key",
        key,
    )
    .unwrap()
}
#[test]
fn recreated_cache_retains_fence_and_checks_expiry_at_use() {
    block_on(async {
        let (store, cache, trust, key) = setup();
        cache.accept(&envelope(&key, 2, 100), 150).await.unwrap();
        let recreated = EventCache::new(store, trust);
        assert!(
            recreated
                .accept(&envelope(&key, 1, 100), 150)
                .await
                .is_err()
        );
        assert!(
            recreated
                .accept(&envelope(&key, 2, 101), 150)
                .await
                .is_err()
        );
        assert!(recreated.current(150).await.unwrap().is_some());
        assert!(recreated.current(200).await.is_err());
    });
}
#[test]
fn concurrent_winner_is_reverified_before_any_overwrite() {
    block_on(async {
        let (store, cache, trust, key) = setup();
        let newer = envelope(&key, 3, 100);
        let verified = lenso_marketplace_catalog::verify(&newer, &trust, None, 150).unwrap();
        store.race.replace(Some(AcceptedEnvelope {
            token: lenso_marketplace_catalog::digest(&newer),
            checkpoint: verified.checkpoint().clone(),
            envelope: newer.clone(),
        }));
        assert!(cache.accept(&envelope(&key, 2, 100), 150).await.is_err());
        assert_eq!(store.value.borrow().as_ref().unwrap().envelope, newer);
    });
}
#[test]
fn storage_failure_never_falls_back_to_event_local_trust_state() {
    block_on(async {
        let (store, cache, _, key) = setup();
        let bytes = envelope(&key, 1, 100);
        cache.accept(&bytes, 150).await.unwrap();
        store.fail.set(true);
        assert!(cache.accept(&bytes, 150).await.is_err());
        assert!(cache.current(150).await.is_err());
    });
}

#[test]
fn expired_browse_acceptance_retains_fences_without_enabling_installation() {
    block_on(async {
        let (store, cache, trust, key) = setup();
        let bytes = envelope(&key, 2, 100);
        assert!(
            cache
                .accept_for_browse(&bytes, 200)
                .await
                .unwrap()
                .is_stale(200)
        );
        let recreated = EventCache::new(store, trust);
        assert!(
            recreated
                .current_for_browse(201)
                .await
                .unwrap()
                .unwrap()
                .is_stale(201)
        );
        assert!(recreated.current(201).await.is_err());
        assert!(recreated.accept(&bytes, 201).await.is_err());
        assert!(
            recreated
                .accept_for_browse(&envelope(&key, 1, 100), 201)
                .await
                .is_err()
        );
        assert!(
            recreated
                .accept_for_browse(&envelope(&key, 2, 101), 201)
                .await
                .is_err()
        );
        assert!(recreated.accept_for_browse(b"invalid", 201).await.is_err());
    });
}
