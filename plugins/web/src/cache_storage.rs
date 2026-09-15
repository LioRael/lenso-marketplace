//! Private persistence seam shared by the Marketplace's native and event hosts.
//! The storage adapter never decides trust or accepts an unverified checkpoint.
use anyhow::{Result, bail};
use futures::future::LocalBoxFuture;
use lenso_plugin_catalog::{
    BrowseSnapshot, Checkpoint, Trust, VerifiedSnapshot, verify, verify_for_browse,
};
use std::{fmt::Debug, rc::Rc};

/// Atomic persisted pair; the opaque token fences concurrent acceptance.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AcceptedEnvelope {
    pub token: String,
    pub checkpoint: Checkpoint,
    #[serde(with = "utf8_envelope")]
    pub envelope: Vec<u8>,
}

/// Host-private operations. Bind one instance explicitly to each event.
pub trait CacheStorage: Debug {
    /// Hint that the next Plan-bound Directory read will be followed by an
    /// acceptance read. Hosts may hand off that read's prior state once only;
    /// subsequent reads (especially after CAS loss) must consult durable storage.
    fn prepare_catalog_read(&self, _catalog: &str) {}

    fn accepted<'a>(
        &'a self,
        catalog: &'a str,
    ) -> LocalBoxFuture<'a, Result<Option<AcceptedEnvelope>>>;
    /// Compare and swap checkpoint and envelope together. `None` means insert
    /// only if absent. A mismatch performs no write and returns false.
    fn compare_exchange<'a>(
        &'a self,
        catalog: &'a str,
        expected: Option<&'a str>,
        value: &'a AcceptedEnvelope,
    ) -> LocalBoxFuture<'a, Result<bool>>;
}

#[derive(Clone, Debug)]
pub struct EventCache {
    storage: Rc<dyn CacheStorage>,
    trust: Trust,
}

impl EventCache {
    pub fn new(storage: Rc<dyn CacheStorage>, trust: Trust) -> Self {
        Self { storage, trust }
    }

    pub async fn accept(&self, envelope: &[u8], now: u64) -> Result<VerifiedSnapshot> {
        self.accept_with(envelope, now, verify, VerifiedSnapshot::checkpoint)
            .await
    }
    pub async fn accept_for_browse(&self, envelope: &[u8], now: u64) -> Result<BrowseSnapshot> {
        self.accept_with(envelope, now, verify_for_browse, BrowseSnapshot::checkpoint)
            .await
    }
    async fn accept_with<T>(
        &self,
        envelope: &[u8],
        now: u64,
        verify: impl Fn(&[u8], &Trust, Option<&Checkpoint>, u64) -> Result<T>,
        checkpoint: impl Fn(&T) -> &Checkpoint,
    ) -> Result<T> {
        // Reverify against the winner after a race, including rollback and
        // same-revision equivocation. Never overwrite a newer accepted state.
        for _ in 0..8 {
            let previous = self.storage.accepted(&self.trust.catalog_id).await?;
            let snapshot = verify(
                envelope,
                &self.trust,
                previous.as_ref().map(|v| &v.checkpoint),
                now,
            )?;
            if previous
                .as_ref()
                .is_some_and(|stored| stored.envelope == envelope)
            {
                return Ok(snapshot);
            }
            let value = AcceptedEnvelope {
                token: lenso_plugin_catalog::digest(envelope),
                checkpoint: checkpoint(&snapshot).clone(),
                envelope: envelope.to_vec(),
            };
            if self
                .storage
                .compare_exchange(
                    &self.trust.catalog_id,
                    previous.as_ref().map(|v| v.token.as_str()),
                    &value,
                )
                .await?
            {
                return Ok(snapshot);
            }
        }
        bail!("catalog acceptance contention exceeded bound")
    }

    pub async fn current(&self, now: u64) -> Result<Option<VerifiedSnapshot>> {
        self.current_with(now, verify).await
    }
    pub async fn current_for_browse(&self, now: u64) -> Result<Option<BrowseSnapshot>> {
        self.current_with(now, verify_for_browse).await
    }
    async fn current_with<T>(
        &self,
        now: u64,
        verify: impl Fn(&[u8], &Trust, Option<&Checkpoint>, u64) -> Result<T>,
    ) -> Result<Option<T>> {
        self.storage
            .accepted(&self.trust.catalog_id)
            .await?
            .map(|stored| verify(&stored.envelope, &self.trust, Some(&stored.checkpoint), now))
            .transpose()
    }
}

mod utf8_envelope {
    use serde::{Deserialize, Deserializer, Serializer, ser::Error};
    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(std::str::from_utf8(bytes).map_err(S::Error::custom)?)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        String::deserialize(deserializer).map(String::into_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use lenso_plugin_catalog::{Snapshot, digest, sign};
    use std::{cell::RefCell, collections::VecDeque};

    #[derive(Debug)]
    struct ScriptedStorage {
        reads: RefCell<VecDeque<Option<AcceptedEnvelope>>>,
        writes: RefCell<Vec<Option<String>>>,
        wins: bool,
    }
    impl CacheStorage for ScriptedStorage {
        fn accepted<'a>(
            &'a self,
            _catalog: &'a str,
        ) -> LocalBoxFuture<'a, Result<Option<AcceptedEnvelope>>> {
            Box::pin(async {
                Ok(self
                    .reads
                    .borrow_mut()
                    .pop_front()
                    .expect("unexpected read"))
            })
        }
        fn compare_exchange<'a>(
            &'a self,
            _catalog: &'a str,
            expected: Option<&'a str>,
            _value: &'a AcceptedEnvelope,
        ) -> LocalBoxFuture<'a, Result<bool>> {
            Box::pin(async move {
                self.writes.borrow_mut().push(expected.map(str::to_owned));
                // Tests with two states lose once, then win after revalidation.
                Ok(self.wins && self.reads.borrow().is_empty())
            })
        }
    }

    fn fixture() -> (Vec<u8>, Trust) {
        let vector: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/support/tests/conformance.json"
        ))
        .unwrap();
        let public = hex::decode(vector["public_key_hex"].as_str().unwrap()).unwrap();
        let trust = Trust {
            catalog_id: "conformance-only".into(),
            keys: std::collections::BTreeMap::from([(
                "test-key".into(),
                ed25519_dalek::VerifyingKey::from_bytes(&public.try_into().unwrap()).unwrap(),
            )]),
        };
        (serde_json::to_vec(&vector["envelope"]).unwrap(), trust)
    }
    fn changed(revision: u64, issued: u64) -> Vec<u8> {
        sign(
            &Snapshot::new("conformance-only".into(), revision, issued, 200, vec![]),
            "test-key",
            &ed25519_dalek::SigningKey::from_bytes(&[17; 32]),
        )
        .unwrap()
    }
    fn accepted(envelope: &[u8], trust: &Trust) -> AcceptedEnvelope {
        AcceptedEnvelope {
            token: digest(envelope),
            checkpoint: verify(envelope, trust, None, 150)
                .unwrap()
                .checkpoint()
                .clone(),
            envelope: envelope.to_vec(),
        }
    }
    fn cache(
        trust: Trust,
        reads: Vec<Option<AcceptedEnvelope>>,
        wins: bool,
    ) -> (EventCache, Rc<ScriptedStorage>) {
        let storage = Rc::new(ScriptedStorage {
            reads: RefCell::new(reads.into()),
            writes: RefCell::default(),
            wins,
        });
        (EventCache::new(storage.clone(), trust), storage)
    }

    #[test]
    fn exact_signed_handoff_is_reverified_without_a_write() {
        let (bytes, trust) = fixture();
        let stored = accepted(&bytes, &trust);
        let (cache, storage) = cache(trust, vec![Some(stored)], true);
        let snapshot = block_on(cache.accept_for_browse(&bytes, 150)).unwrap();
        assert_eq!(snapshot.snapshot().revision, 1);
        assert!(storage.reads.borrow().is_empty());
        assert!(storage.writes.borrow().is_empty());
    }

    #[test]
    fn identical_bytes_do_not_bypass_trust_checkpoint_or_signature_checks() {
        let (bytes, trust) = fixture();
        for kind in ["trust", "checkpoint", "signature"] {
            let mut trust = trust.clone();
            let mut stored = accepted(&bytes, &trust);
            if kind == "trust" {
                trust.keys.clear();
            } else if kind == "checkpoint" {
                stored.checkpoint.payload_digest = digest(b"corrupt");
            } else {
                let mut envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                envelope["signature_base64"] = serde_json::json!("A".repeat(86) + "==");
                stored.envelope = serde_json::to_vec(&envelope).unwrap();
                stored.token = digest(&stored.envelope);
            }
            let candidate = stored.envelope.clone();
            let (cache, storage) = cache(trust, vec![Some(stored)], true);
            assert!(
                block_on(cache.accept_for_browse(&candidate, 150)).is_err(),
                "{kind}"
            );
            assert!(storage.writes.borrow().is_empty());
        }
    }

    #[test]
    fn missing_and_changed_state_keep_the_initial_cas_fence() {
        let (bytes, trust) = fixture();
        for prior in [None, Some(accepted(&bytes, &trust))] {
            let expected = prior.as_ref().map(|value| value.token.clone());
            let (cache, storage) = cache(trust.clone(), vec![prior], true);
            let snapshot = block_on(cache.accept_for_browse(&changed(2, 100), 150)).unwrap();
            assert_eq!(snapshot.snapshot().revision, 2);
            assert_eq!(*storage.writes.borrow(), vec![expected]);
        }
    }

    #[test]
    fn cas_loss_revalidates_the_durable_winner_and_retries_with_its_token() {
        let (bytes, trust) = fixture();
        let prior = accepted(&bytes, &trust);
        let candidate = changed(3, 100);
        for (winner_bytes, succeeds, writes) in [
            (changed(4, 100), false, 1),  // Rollback against a newer winner.
            (changed(3, 101), false, 1),  // Same revision equivocation.
            (candidate.clone(), true, 1), // Identical winner: verify, no rewrite.
            (changed(2, 100), true, 2),   // Older winner: reverify and retry CAS.
        ] {
            let winner = accepted(&winner_bytes, &trust);
            let winner_token = winner.token.clone();
            let (cache, storage) =
                cache(trust.clone(), vec![Some(prior.clone()), Some(winner)], true);
            assert_eq!(
                block_on(cache.accept_for_browse(&candidate, 150)).is_ok(),
                succeeds
            );
            assert!(storage.reads.borrow().is_empty());
            let actual = storage.writes.borrow();
            assert_eq!(actual.len(), writes);
            assert_eq!(actual[0], Some(prior.token.clone()));
            if writes == 2 {
                assert_eq!(actual[1], Some(winner_token));
            }
        }
    }

    #[test]
    fn repeated_cas_loss_remains_bounded() {
        let (bytes, trust) = fixture();
        let prior = accepted(&bytes, &trust);
        let (cache, storage) = cache(trust, vec![Some(prior); 8], false);
        let error = block_on(cache.accept_for_browse(&changed(2, 100), 150)).unwrap_err();
        assert!(error.to_string().contains("contention exceeded bound"));
        assert_eq!(storage.writes.borrow().len(), 8);
        assert!(storage.reads.borrow().is_empty());
    }
}
