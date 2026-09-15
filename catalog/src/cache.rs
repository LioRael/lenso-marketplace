//! A consumer-owned verified cache. Checkpoint and snapshot commit together.
use crate::{BrowseSnapshot, Checkpoint, Trust, VerifiedSnapshot, verify, verify_for_browse};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior, params};
use std::path::Path;

#[derive(Debug)]
pub struct VerifiedCache {
    connection: Connection,
    trust: Trust,
}
impl VerifiedCache {
    pub fn open(path: &Path, trust: Trust) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS accepted_catalog (catalog_id TEXT PRIMARY KEY, checkpoint TEXT NOT NULL, envelope BLOB NOT NULL)")?;
        Ok(Self { connection, trust })
    }
    pub fn accept(&mut self, envelope: &[u8], now: u64) -> Result<VerifiedSnapshot> {
        self.accept_with(envelope, now, verify, VerifiedSnapshot::checkpoint)
    }
    pub fn accept_for_browse(&mut self, envelope: &[u8], now: u64) -> Result<BrowseSnapshot> {
        self.accept_with(envelope, now, verify_for_browse, BrowseSnapshot::checkpoint)
    }
    fn accept_with<T>(
        &mut self,
        envelope: &[u8],
        now: u64,
        verify: impl Fn(&[u8], &Trust, Option<&Checkpoint>, u64) -> Result<T>,
        checkpoint: impl Fn(&T) -> &Checkpoint,
    ) -> Result<T> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<String> = transaction
            .query_row(
                "SELECT checkpoint FROM accepted_catalog WHERE catalog_id=?1",
                [&self.trust.catalog_id],
                |row| row.get(0),
            )
            .optional()?;
        let previous = previous
            .map(|json| serde_json::from_str::<Checkpoint>(&json))
            .transpose()?;
        let snapshot = verify(envelope, &self.trust, previous.as_ref(), now)?;
        transaction.execute("INSERT INTO accepted_catalog VALUES(?1,?2,?3) ON CONFLICT(catalog_id) DO UPDATE SET checkpoint=excluded.checkpoint,envelope=excluded.envelope",params![self.trust.catalog_id,serde_json::to_string(checkpoint(&snapshot))?,envelope])?;
        transaction.commit()?;
        Ok(snapshot)
    }
    pub fn current(&self, now: u64) -> Result<Option<VerifiedSnapshot>> {
        self.current_with(now, verify)
    }
    pub fn current_for_browse(&self, now: u64) -> Result<Option<BrowseSnapshot>> {
        self.current_with(now, verify_for_browse)
    }
    fn current_with<T>(
        &self,
        now: u64,
        verify: impl Fn(&[u8], &Trust, Option<&Checkpoint>, u64) -> Result<T>,
    ) -> Result<Option<T>> {
        let stored: Option<(String, Vec<u8>)> = self
            .connection
            .query_row(
                "SELECT checkpoint,envelope FROM accepted_catalog WHERE catalog_id=?1",
                [&self.trust.catalog_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        stored
            .map(|(checkpoint, envelope)| {
                verify(
                    &envelope,
                    &self.trust,
                    Some(&serde_json::from_str(&checkpoint)?),
                    now,
                )
            })
            .transpose()
    }
}
