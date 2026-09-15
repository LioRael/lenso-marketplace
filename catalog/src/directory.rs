//! Durable curated publishing domain. Callers obtain actor IDs from authenticated
//! invocation context, never publisher JSON. HTTP/Auth integration is a separate seam.
use crate::{Availability, Release, Snapshot, digest, sign};
use anyhow::{Context, Result, ensure};
use ed25519_dalek::SigningKey;
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior, params};
use std::{collections::BTreeSet, path::Path};

pub struct Directory {
    connection: Connection,
    reviewers: BTreeSet<String>,
    catalog_id: String,
}

/// Read-only projection for the public directory Plugin. Opening this handle never
/// creates a database, schema, namespace, submission or signing key.
#[derive(Debug)]
pub struct PublishedDirectory {
    connection: Connection,
}

impl PublishedDirectory {
    pub fn open(path: &Path, catalog_id: &str) -> Result<Self> {
        let connection =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let existing: String = connection.query_row(
            "SELECT catalog_id FROM metadata WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        ensure!(
            existing == catalog_id,
            "database belongs to another catalog"
        );
        Ok(Self { connection })
    }

    /// Write a consistent standalone SQLite image to a new operator-owned path.
    /// Failed attempts leave the destination for inspection; never overwrite it.
    pub fn backup(&self, destination: &Path) -> Result<()> {
        ensure!(destination.is_absolute(), "backup path must be absolute");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let file = options.open(destination)?;
        let mut target = Connection::open(destination)?;
        {
            let backup = rusqlite::backup::Backup::new(&self.connection, &mut target)?;
            ensure!(
                backup.step(-1)? == rusqlite::backup::StepResult::Done,
                "backup incomplete; preserve destination and retry with a new path"
            );
        }
        let integrity: String = target.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        ensure!(integrity == "ok", "backup integrity check failed");
        target.close().map_err(|(_, error)| error)?;
        file.sync_all()?;
        if let Some(parent) = destination.parent() {
            std::fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    }

    pub fn latest(&self) -> Result<Option<String>> {
        let value: Option<Option<Vec<u8>>> = self.connection.query_row(
            "SELECT CASE WHEN length(envelope)<=?1 THEN envelope ELSE NULL END FROM snapshots ORDER BY revision DESC LIMIT 1",
            [i64::try_from(crate::MAX_ENVELOPE_BYTES)?], |row| row.get(0)
        ).optional()?;
        match value {
            None => Ok(None),
            Some(None) => anyhow::bail!("published envelope exceeds size limit"),
            Some(Some(bytes)) => Ok(Some(String::from_utf8(bytes)?)),
        }
    }
}

impl Directory {
    pub fn open(path: &Path, catalog_id: &str, reviewers: BTreeSet<String>) -> Result<Self> {
        super::bounded_text(catalog_id, 128)?;
        ensure!(!reviewers.is_empty(), "at least one reviewer is required");
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS metadata (singleton INTEGER PRIMARY KEY CHECK(singleton=1), catalog_id TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS namespaces (namespace TEXT PRIMARY KEY, publisher TEXT NOT NULL, actor TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS submissions (id INTEGER PRIMARY KEY, identity TEXT NOT NULL UNIQUE, publisher TEXT NOT NULL, body TEXT NOT NULL, digest TEXT NOT NULL, state TEXT NOT NULL, reviewer TEXT, policy TEXT);
            CREATE TABLE IF NOT EXISTS snapshots (revision INTEGER PRIMARY KEY, envelope BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS audit (id INTEGER PRIMARY KEY, actor TEXT NOT NULL, action TEXT NOT NULL, subject TEXT NOT NULL, at INTEGER NOT NULL);
        ")?;
        connection.execute("INSERT OR IGNORE INTO metadata VALUES(1, ?1)", [catalog_id])?;
        let existing: String =
            connection.query_row("SELECT catalog_id FROM metadata", [], |row| row.get(0))?;
        ensure!(
            existing == catalog_id,
            "database belongs to another catalog"
        );
        Ok(Self {
            connection,
            catalog_id: catalog_id.into(),
            reviewers,
        })
    }

    fn authorize_review(&self, actor: &str) -> Result<()> {
        ensure!(
            self.reviewers.contains(actor),
            "reviewer authorization required"
        );
        Ok(())
    }

    pub fn claim_namespace(
        &mut self,
        actor: &str,
        namespace: &str,
        publisher: &str,
        publisher_actor: &str,
        now: u64,
    ) -> Result<()> {
        self.authorize_review(actor)?;
        lenso_plugin_catalog::identity::validate_plugin_id_v1(&format!("{namespace}.claim"))?;
        super::bounded_text(publisher, 128)?;
        super::bounded_text(publisher_actor, 128)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Vec<String> = transaction
            .prepare("SELECT namespace FROM namespaces")?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        ensure!(
            !existing.iter().any(|other| other == namespace
                || other.starts_with(&format!("{namespace}."))
                || namespace.starts_with(&format!("{other}."))),
            "namespace overlaps an existing claim"
        );
        transaction.execute(
            "INSERT INTO namespaces VALUES(?1,?2,?3)",
            params![namespace, publisher, publisher_actor],
        )?;
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'claim',?2,?3)",
            params![actor, namespace, i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Ingestion must own a private extraction of the exact archive supplied here.
    /// This method performs no network fetch or archive extraction.
    pub fn submit(
        &mut self,
        actor: &str,
        release: &Release,
        archive: &[u8],
        extracted: &Path,
        now: u64,
    ) -> Result<i64> {
        release.verify_archive_bytes(archive)?;
        release.verify_bundle_directory(extracted)?;
        self.insert_verified(actor, release, now)
    }

    fn insert_verified(&mut self, actor: &str, release: &Release, now: u64) -> Result<i64> {
        release.validate()?;
        ensure!(
            release.availability == Availability::Listed,
            "new submissions must be listed candidates"
        );
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let claims: Vec<String> = transaction
            .prepare("SELECT namespace FROM namespaces WHERE publisher=?1 AND actor=?2")?
            .query_map(params![release.publisher_id, actor], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        ensure!(
            claims
                .iter()
                .any(|namespace| release.plugin_id.starts_with(&format!("{namespace}."))),
            "publisher does not own this namespace"
        );
        let body = serde_json::to_string(release)?;
        let body_digest = digest(body.as_bytes());
        let identity = format!("{}@{}", release.plugin_id, release.version);
        let existing: Option<(i64, String)> = transaction
            .query_row(
                "SELECT id,digest FROM submissions WHERE identity=?1",
                [&identity],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((id, previous)) = existing {
            ensure!(
                previous == body_digest,
                "release identity already submitted with different content"
            );
            return Ok(id);
        }
        let count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM submissions", [], |row| row.get(0))?;
        ensure!(
            count < i64::try_from(super::MAX_RELEASES)?,
            "directory submission limit reached"
        );
        transaction.execute("INSERT INTO submissions(identity,publisher,body,digest,state) VALUES(?1,?2,?3,?4,'awaiting_review')", params![identity, release.publisher_id, body, body_digest])?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'submit',?2,?3)",
            params![actor, identity, i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn approve(
        &mut self,
        actor: &str,
        submission: i64,
        expected_digest: &str,
        policy: &str,
        now: u64,
    ) -> Result<()> {
        self.authorize_review(actor)?;
        super::bounded_text(policy, 128)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute("UPDATE submissions SET state='approved',reviewer=?1,policy=?2 WHERE id=?3 AND digest=?4 AND state='awaiting_review'", params![actor, policy, submission, expected_digest])?;
        ensure!(changed == 1, "submission changed or is not awaiting review");
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'approve',?2,?3)",
            params![actor, submission.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn inspect_submission(
        &self,
        actor: &str,
        submission: i64,
    ) -> Result<(Release, String, String)> {
        let (body, digest, state, publisher): (String, String, String, String) =
            self.connection.query_row(
                "SELECT body,digest,state,publisher FROM submissions WHERE id=?1",
                [submission],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        let owner: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM namespaces WHERE publisher=?1 AND actor=?2)",
            params![publisher, actor],
            |row| row.get(0),
        )?;
        ensure!(
            owner || self.reviewers.contains(actor),
            "submission access denied"
        );
        Ok((serde_json::from_str(&body)?, digest, state))
    }

    /// Publication and signed snapshot are one durable transaction. Expected revision
    /// fences competing publishers. After an uncertain response, read `latest`.
    pub fn publish(
        &mut self,
        actor: &str,
        expected_revision: u64,
        now: u64,
        expires_at: u64,
        key_id: &str,
        key: &SigningKey,
    ) -> Result<Vec<u8>> {
        self.authorize_review(actor)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM snapshots",
            [],
            |row| row.get(0),
        )?;
        ensure!(
            current == i64::try_from(expected_revision)?,
            "catalog revision changed; read publication result"
        );
        let bodies: Vec<String> = transaction.prepare("SELECT body FROM submissions WHERE state IN ('approved','published') ORDER BY identity")?.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
        let releases = bodies
            .iter()
            .map(|body| serde_json::from_str(body))
            .collect::<serde_json::Result<Vec<Release>>>()?;
        let revision = current
            .checked_add(1)
            .context("catalog revision overflow")?;
        let envelope = sign(
            &Snapshot::new(
                self.catalog_id.clone(),
                u64::try_from(revision)?,
                now,
                expires_at,
                releases,
            ),
            key_id,
            key,
        )?;
        transaction.execute(
            "INSERT INTO snapshots VALUES(?1,?2)",
            params![revision, envelope],
        )?;
        transaction.execute(
            "UPDATE submissions SET state='published' WHERE state='approved'",
            [],
        )?;
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'publish',?2,?3)",
            params![actor, revision.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(envelope)
    }

    pub fn latest(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .connection
            .query_row(
                "SELECT envelope FROM snapshots ORDER BY revision DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release() -> Release {
        Release {
            plugin_id: "example.echo".into(),
            version: "0.1.0".into(),
            publisher_id: "publisher".into(),
            title: "Echo".into(),
            summary: "Return text".into(),
            description: String::new(),
            presentation: None,
            source_url: "https://example.test/source".into(),
            source_revision: "a".repeat(40),
            license: "MIT".into(),
            artifact: crate::Artifact {
                url: "https://example.test/plugin".into(),
                digest: digest(b"test"),
                size: 4,
                manifest_digest: digest(b"manifest"),
            },
            availability: Availability::Listed,
        }
    }
    #[test]
    fn ownership_approval_and_snapshot_survive_restart() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("directory.db");
        let reviewers = BTreeSet::from(["reviewer".into()]);
        let mut directory = Directory::open(&path, "test", reviewers.clone()).unwrap();
        assert!(
            directory
                .claim_namespace("stranger", "example", "publisher", "author", 100)
                .is_err()
        );
        directory
            .claim_namespace("reviewer", "example", "publisher", "author", 100)
            .unwrap();
        assert!(
            directory
                .claim_namespace("reviewer", "example.sub", "other", "other", 100)
                .is_err()
        );
        assert!(
            directory
                .insert_verified("stranger", &release(), 101)
                .is_err()
        );
        // Only this domain test bypasses the separate executable Bundle verifier.
        let id = directory
            .insert_verified("author", &release(), 101)
            .unwrap();
        assert_eq!(
            directory
                .insert_verified("author", &release(), 101)
                .unwrap(),
            id
        );
        assert!(directory.inspect_submission("stranger", id).is_err());
        let (_, digest, _) = directory.inspect_submission("author", id).unwrap();
        assert!(directory.approve("author", id, &digest, "v1", 102).is_err());
        assert!(
            directory
                .approve("reviewer", id, "wrong", "v1", 102)
                .is_err()
        );
        let key = SigningKey::from_bytes(&[17; 32]);
        let first = directory
            .publish("reviewer", 0, 103, 200, "test-key", &key)
            .unwrap();
        let trust = crate::Trust {
            catalog_id: "test".into(),
            keys: std::collections::BTreeMap::from([("test-key".into(), key.verifying_key())]),
        };
        assert!(
            crate::verify(&first, &trust, None, 150)
                .unwrap()
                .snapshot()
                .releases
                .is_empty()
        );
        directory
            .approve("reviewer", id, &digest, "v1", 104)
            .unwrap();
        let published = directory
            .publish("reviewer", 1, 105, 200, "test-key", &key)
            .unwrap();
        assert!(
            directory
                .publish("reviewer", 1, 105, 200, "test-key", &key)
                .is_err()
        );
        drop(directory);
        let mut reopened = Directory::open(&path, "test", reviewers).unwrap();
        assert_eq!(reopened.latest().unwrap().unwrap(), published);
        let verified = crate::verify(&published, &trust, None, 150).unwrap();
        assert_eq!(verified.snapshot().releases.len(), 1);
        assert_eq!(
            reopened.inspect_submission("author", id).unwrap().2,
            "published"
        );
        let mut changed = release();
        changed.artifact.digest = crate::digest(b"changed");
        assert!(reopened.insert_verified("author", &changed, 110).is_err());
    }
}
