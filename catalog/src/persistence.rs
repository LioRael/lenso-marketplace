//! Private persistence seam shared by the Marketplace's native and event hosts.
//! The storage adapter never decides trust or accepts an unverified checkpoint.
use crate::{Checkpoint, Trust, VerifiedSnapshot, verify};
use anyhow::{Result, bail};
use futures::future::LocalBoxFuture;
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
pub trait SnapshotStorage: Debug {
    fn published<'a>(&'a self, catalog: &'a str) -> LocalBoxFuture<'a, Result<Option<String>>>;
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
    storage: Rc<dyn SnapshotStorage>,
    trust: Trust,
}

impl EventCache {
    pub fn new(storage: Rc<dyn SnapshotStorage>, trust: Trust) -> Self {
        Self { storage, trust }
    }

    pub async fn accept(&self, envelope: &[u8], now: u64) -> Result<VerifiedSnapshot> {
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
                token: crate::digest(envelope),
                checkpoint: snapshot.checkpoint().clone(),
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
