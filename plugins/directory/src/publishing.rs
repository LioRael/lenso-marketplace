//! Durable curated publishing domain. Callers obtain actor IDs from authenticated
//! invocation context, never publisher JSON. HTTP/Auth integration is a separate seam.
use anyhow::{Context, Result, ensure};
use ed25519_dalek::SigningKey;
use lenso_plugin_catalog::{
    Availability, Release, ReleaseDetails, ReleaseDetailsSnapshot, Snapshot, digest, sign,
    sign_release_details,
};
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior, params};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

mod details_revision;
mod linked_cargo;
mod package;
pub mod release_content;
mod release_content_source;
use details_revision::ensure_additive_documents;
pub use linked_cargo::{LinkedCargoCrateIdentity, linked_cargo_crate_identity};
pub use package::MAX_NPM_ARCHIVE_BYTES;

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
            [i64::try_from(lenso_plugin_catalog::MAX_ENVELOPE_BYTES)?], |row| row.get(0)
        ).optional()?;
        match value {
            None => Ok(None),
            Some(None) => anyhow::bail!("published envelope exceeds size limit"),
            Some(Some(bytes)) => Ok(Some(String::from_utf8(bytes)?)),
        }
    }

    pub fn latest_details(&self) -> Result<Option<String>> {
        let value: Option<Option<Vec<u8>>> = self.connection.query_row(
            "SELECT CASE WHEN length(envelope)<=?1 THEN envelope ELSE NULL END FROM details_snapshots ORDER BY revision DESC LIMIT 1",
            [i64::try_from(lenso_plugin_catalog::MAX_ENVELOPE_BYTES)?], |row| row.get(0)
        ).optional()?;
        match value {
            None => Ok(None),
            Some(None) => anyhow::bail!("published release details exceed size limit"),
            Some(Some(bytes)) => Ok(Some(String::from_utf8(bytes)?)),
        }
    }

    pub fn latest_linked_cargo(&self) -> Result<Option<String>> {
        let value: Option<Option<Vec<u8>>> = self.connection.query_row(
            "SELECT CASE WHEN length(envelope)<=?1 THEN envelope ELSE NULL END FROM linked_cargo_snapshots ORDER BY revision DESC LIMIT 1",
            [i64::try_from(lenso_plugin_catalog::MAX_ENVELOPE_BYTES)?], |row| row.get(0)
        ).optional()?;
        match value {
            None => Ok(None),
            Some(None) => anyhow::bail!("published linked Cargo envelope exceeds size limit"),
            Some(Some(bytes)) => Ok(Some(String::from_utf8(bytes)?)),
        }
    }

    pub fn latest_package(&self) -> Result<Option<String>> {
        // A read-only projection may open a database from before this channel existed.
        let has_table: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='package_snapshots')",
            [],
            |row| row.get(0),
        )?;
        if !has_table {
            return Ok(None);
        }
        let value: Option<Option<Vec<u8>>> = self.connection.query_row(
            "SELECT CASE WHEN length(envelope)<=?1 THEN envelope ELSE NULL END FROM package_snapshots ORDER BY revision DESC LIMIT 1",
            [i64::try_from(lenso_plugin_catalog::MAX_ENVELOPE_BYTES)?], |row| row.get(0)
        ).optional()?;
        match value {
            None => Ok(None),
            Some(None) => anyhow::bail!("published package envelope exceeds size limit"),
            Some(Some(bytes)) => Ok(Some(String::from_utf8(bytes)?)),
        }
    }

    pub fn latest_release_content(&self) -> Result<Option<String>> {
        // Existing publisher databases predate this optional publication channel.
        // The read-only public projection must not run a migration on request.
        let has_table: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='release_content_snapshots')",
            [],
            |row| row.get(0),
        )?;
        if !has_table {
            return Ok(None);
        }
        let value: Option<Option<Vec<u8>>> = self.connection.query_row(
            "SELECT CASE WHEN length(envelope)<=?1 THEN envelope ELSE NULL END FROM release_content_snapshots ORDER BY revision DESC LIMIT 1",
            [i64::try_from(lenso_plugin_catalog::MAX_ENVELOPE_BYTES)?], |row| row.get(0)
        ).optional()?;
        match value {
            None => Ok(None),
            Some(None) => anyhow::bail!("published release content envelope exceeds size limit"),
            Some(Some(bytes)) => Ok(Some(String::from_utf8(bytes)?)),
        }
    }
}

impl Directory {
    pub fn open(path: &Path, catalog_id: &str, reviewers: BTreeSet<String>) -> Result<Self> {
        lenso_plugin_catalog::bounded_text(catalog_id, 128)?;
        ensure!(!reviewers.is_empty(), "at least one reviewer is required");
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS metadata (singleton INTEGER PRIMARY KEY CHECK(singleton=1), catalog_id TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS namespaces (namespace TEXT PRIMARY KEY, publisher TEXT NOT NULL, actor TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS submissions (id INTEGER PRIMARY KEY, identity TEXT NOT NULL UNIQUE, publisher TEXT NOT NULL, body TEXT NOT NULL, digest TEXT NOT NULL, state TEXT NOT NULL, reviewer TEXT, policy TEXT);
            CREATE TABLE IF NOT EXISTS snapshots (revision INTEGER PRIMARY KEY, envelope BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS details_submissions (id INTEGER PRIMARY KEY, identity TEXT NOT NULL UNIQUE, publisher TEXT NOT NULL, body TEXT NOT NULL, digest TEXT NOT NULL, state TEXT NOT NULL, reviewer TEXT, policy TEXT);
            CREATE TABLE IF NOT EXISTS details_amendments (id INTEGER PRIMARY KEY, identity TEXT NOT NULL, publisher TEXT NOT NULL, body TEXT NOT NULL, digest TEXT NOT NULL, base_digest TEXT NOT NULL, state TEXT NOT NULL, reviewer TEXT, policy TEXT);
            CREATE UNIQUE INDEX IF NOT EXISTS details_amendments_identity_digest ON details_amendments(identity,digest);
            CREATE TABLE IF NOT EXISTS details_snapshots (revision INTEGER PRIMARY KEY, envelope BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS linked_cargo_submissions (id INTEGER PRIMARY KEY, identity TEXT NOT NULL UNIQUE, publisher TEXT NOT NULL, body TEXT NOT NULL, digest TEXT NOT NULL, state TEXT NOT NULL, reviewer TEXT, policy TEXT);
            CREATE TABLE IF NOT EXISTS linked_cargo_amendments (id INTEGER PRIMARY KEY, identity TEXT NOT NULL, publisher TEXT NOT NULL, body TEXT NOT NULL, digest TEXT NOT NULL, base_digest TEXT NOT NULL, state TEXT NOT NULL, reviewer TEXT, policy TEXT);
            CREATE UNIQUE INDEX IF NOT EXISTS linked_cargo_amendments_identity_digest ON linked_cargo_amendments(identity,digest);
            CREATE TABLE IF NOT EXISTS linked_cargo_snapshots (revision INTEGER PRIMARY KEY, envelope BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS package_submissions (id INTEGER PRIMARY KEY, identity TEXT NOT NULL UNIQUE, publisher TEXT NOT NULL, body TEXT NOT NULL, digest TEXT NOT NULL, state TEXT NOT NULL, reviewer TEXT, policy TEXT);
            CREATE TABLE IF NOT EXISTS package_snapshots (revision INTEGER PRIMARY KEY, envelope BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS release_content_submissions (id INTEGER PRIMARY KEY, identity TEXT NOT NULL UNIQUE, publisher TEXT NOT NULL, body TEXT NOT NULL, digest TEXT NOT NULL, state TEXT NOT NULL, reviewer TEXT, policy TEXT);
            CREATE TABLE IF NOT EXISTS release_content_snapshots (revision INTEGER PRIMARY KEY, envelope BLOB NOT NULL);
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

    fn ensure_exclusive_release_identities(transaction: &rusqlite::Transaction<'_>) -> Result<()> {
        let collision: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM submissions p JOIN linked_cargo_submissions l ON p.identity=l.identity
                UNION ALL SELECT 1 FROM submissions p JOIN package_submissions n ON p.identity=n.identity
                UNION ALL SELECT 1 FROM linked_cargo_submissions l JOIN package_submissions n ON l.identity=n.identity)",
            [],
            |row| row.get(0),
        )?;
        ensure!(
            !collision,
            "release identity is owned by multiple base channels"
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
        lenso_plugin_catalog::bounded_text(publisher, 128)?;
        lenso_plugin_catalog::bounded_text(publisher_actor, 128)?;
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
        let existing_linked: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM linked_cargo_submissions WHERE identity=?1)",
            [&identity],
            |row| row.get(0),
        )?;
        ensure!(
            !existing_linked,
            "a source-only linked Cargo release already owns this identity"
        );
        let existing_package: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM package_submissions WHERE identity=?1)",
            [&identity],
            |row| row.get(0),
        )?;
        ensure!(
            !existing_package,
            "a package-only release already owns this identity"
        );
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
            count < i64::try_from(lenso_plugin_catalog::MAX_RELEASES)?,
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
        lenso_plugin_catalog::bounded_text(policy, 128)?;
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

    pub fn submit_details(
        &mut self,
        actor: &str,
        details: &ReleaseDetails,
        now: u64,
    ) -> Result<i64> {
        details.validate()?;
        let identity = format!("{}@{}", details.plugin_id, details.version);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (base_body, publisher, base_state): (String, String, String) = transaction.query_row(
            "SELECT body,publisher,state FROM submissions WHERE identity=?1",
            [&identity],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        ensure!(
            base_state == "published",
            "release details require an already published base release"
        );
        let base: Release = serde_json::from_str(&base_body)?;
        details.validate_against(&base)?;
        let owns_namespace: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM namespaces WHERE publisher=?1 AND actor=?2)",
            params![publisher, actor],
            |row| row.get(0),
        )?;
        ensure!(owns_namespace, "publisher does not own this namespace");
        let body = serde_json::to_string(details)?;
        let body_digest = digest(body.as_bytes());
        let existing: Option<(i64, String)> = transaction
            .query_row(
                "SELECT id,digest FROM details_submissions WHERE identity=?1",
                [&identity],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((id, previous)) = existing {
            ensure!(
                previous == body_digest,
                "release details identity already submitted with different content"
            );
            return Ok(id);
        }
        transaction.execute(
            "INSERT INTO details_submissions(identity,publisher,body,digest,state) VALUES(?1,?2,?3,?4,'awaiting_review')",
            params![identity, publisher, body, body_digest],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'submit_details',?2,?3)",
            params![actor, identity, i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn inspect_details_submission(
        &self,
        actor: &str,
        submission: i64,
    ) -> Result<(ReleaseDetails, String, String)> {
        let (body, digest, state, publisher): (String, String, String, String) =
            self.connection.query_row(
                "SELECT body,digest,state,publisher FROM details_submissions WHERE id=?1",
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
            "release details access denied"
        );
        Ok((serde_json::from_str(&body)?, digest, state))
    }

    pub fn approve_details(
        &mut self,
        actor: &str,
        submission: i64,
        expected_digest: &str,
        policy: &str,
        now: u64,
    ) -> Result<()> {
        self.authorize_review(actor)?;
        lenso_plugin_catalog::bounded_text(policy, 128)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE details_submissions SET state='approved',reviewer=?1,policy=?2 WHERE id=?3 AND digest=?4 AND state='awaiting_review'",
            params![actor, policy, submission, expected_digest],
        )?;
        ensure!(
            changed == 1,
            "release details changed or are not awaiting review"
        );
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'approve_details',?2,?3)",
            params![actor, submission.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(())
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
        Self::ensure_exclusive_release_identities(&transaction)?;
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

    pub fn publish_details(
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
            "SELECT COALESCE(MAX(revision),0) FROM details_snapshots",
            [],
            |row| row.get(0),
        )?;
        ensure!(
            current == i64::try_from(expected_revision)?,
            "release details revision changed; read publication result"
        );
        let bodies: Vec<(String, String)> = transaction
            .prepare("SELECT body,digest FROM details_submissions WHERE state IN ('approved','published') ORDER BY identity")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut releases = BTreeMap::<String, (ReleaseDetails, String)>::new();
        for (body, reviewed_digest) in &bodies {
            ensure!(
                digest(body.as_bytes()) == *reviewed_digest,
                "reviewed release details digest mismatch"
            );
            let details: ReleaseDetails = serde_json::from_str(body)?;
            let identity = format!("{}@{}", details.plugin_id, details.version);
            ensure!(
                releases
                    .insert(identity, (details, digest(body.as_bytes())))
                    .is_none(),
                "duplicate release details identity"
            );
        }
        let amendments: Vec<(String, String, String, String, String)> = transaction
            .prepare("SELECT identity,body,digest,base_digest,state FROM details_amendments WHERE state IN ('approved','published') ORDER BY id")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut approved = BTreeSet::new();
        for (identity, body, reviewed_digest, base_digest, state) in amendments {
            ensure!(
                digest(body.as_bytes()) == reviewed_digest,
                "reviewed documentation revision digest mismatch"
            );
            let (previous, previous_digest) = releases
                .get(&identity)
                .context("documentation revision has no published base")?;
            ensure!(
                previous_digest == &base_digest,
                "documentation revision base changed"
            );
            let details: ReleaseDetails = serde_json::from_str(&body)?;
            details.validate()?;
            ensure_additive_documents(previous, &details)?;
            if state == "approved" {
                ensure!(
                    approved.insert(identity.clone()),
                    "multiple approved documentation revisions"
                );
            }
            releases.insert(identity, (details, digest(body.as_bytes())));
        }
        let releases: Vec<ReleaseDetails> =
            releases.into_values().map(|(details, _)| details).collect();
        let revision = current
            .checked_add(1)
            .context("release details revision overflow")?;
        let envelope = sign_release_details(
            &ReleaseDetailsSnapshot::new(
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
            "INSERT INTO details_snapshots VALUES(?1,?2)",
            params![revision, envelope],
        )?;
        transaction.execute(
            "UPDATE details_submissions SET state='published' WHERE state='approved'",
            [],
        )?;
        transaction.execute(
            "UPDATE details_amendments SET state='published' WHERE state='approved'",
            [],
        )?;
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'publish_details',?2,?3)",
            params![actor, revision.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(envelope)
    }

    pub fn latest_details(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .connection
            .query_row(
                "SELECT envelope FROM details_snapshots ORDER BY revision DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lenso_plugin_catalog::{Distribution, DistributionKind, Documentation};
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
            artifact: lenso_plugin_catalog::Artifact {
                url: "https://example.test/plugin".into(),
                digest: digest(b"test"),
                size: 4,
                manifest_digest: digest(b"manifest"),
            },
            availability: Availability::Listed,
        }
    }
    fn details(base: &Release) -> ReleaseDetails {
        ReleaseDetails {
            plugin_id: base.plugin_id.clone(),
            version: base.version.clone(),
            base_release_identity: base.immutable_identity().unwrap(),
            distributions: vec![
                Distribution {
                    id: "portable".into(),
                    kind: DistributionKind::PortableBundle,
                    package: base.plugin_id.clone(),
                    version: base.version.clone(),
                    integrity: None,
                    registry_url: None,
                    artifact: Some(base.artifact.clone()),
                    targets: vec![],
                },
                Distribution {
                    id: "linked-rust".into(),
                    kind: DistributionKind::CargoPackage,
                    package: "lenso-example-echo-plugin".into(),
                    version: base.version.clone(),
                    integrity: Some(digest(b"crate archive")),
                    registry_url: Some("https://crates.io".into()),
                    artifact: None,
                    targets: vec!["aarch64-apple-darwin".into()],
                },
            ],
            documentation: vec![],
        }
    }
    fn document(revision: &str) -> Documentation {
        Documentation {
            id: "getting-started".into(),
            revision: revision.into(),
            language: "en".into(),
            topic: "start".into(),
            target: None,
            url: format!("https://example.test/docs/0.1.0/getting-started/{revision}"),
            digest: digest(revision.as_bytes()),
            size: revision.len() as u64,
            media_type: "text/markdown".into(),
        }
    }
    #[test]
    fn portable_release_cannot_reuse_source_only_identity() {
        let home = tempfile::tempdir().unwrap();
        let mut directory = Directory::open(
            &home.path().join("directory.db"),
            "test",
            BTreeSet::from(["reviewer".into()]),
        )
        .unwrap();
        directory
            .claim_namespace("reviewer", "example", "publisher", "author", 100)
            .unwrap();
        directory
            .connection
            .execute(
                "INSERT INTO linked_cargo_submissions(identity,publisher,body,digest,state) VALUES(?1,?2,?3,?4,'awaiting_review')",
                params!["example.echo@0.1.0", "publisher", "{}", digest(b"{}")],
            )
            .unwrap();
        assert!(
            directory
                .insert_verified("author", &release(), 101)
                .is_err()
        );
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
        let trust = lenso_plugin_catalog::Trust {
            catalog_id: "test".into(),
            keys: std::collections::BTreeMap::from([("test-key".into(), key.verifying_key())]),
        };
        assert!(
            lenso_plugin_catalog::verify(&first, &trust, None, 150)
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
        let verified = lenso_plugin_catalog::verify(&published, &trust, None, 150).unwrap();
        assert_eq!(verified.snapshot().releases.len(), 1);
        let mut release_details = details(&release());
        release_details.documentation.push(document("r1"));
        let details_id = reopened
            .submit_details("author", &release_details, 106)
            .unwrap();
        let (_, details_digest, details_state) = reopened
            .inspect_details_submission("author", details_id)
            .unwrap();
        assert_eq!(details_state, "awaiting_review");
        reopened
            .approve_details("reviewer", details_id, &details_digest, "v1", 107)
            .unwrap();
        let details_envelope = reopened
            .publish_details("reviewer", 0, 108, 200, "test-key", &key)
            .unwrap();
        let verified_details =
            lenso_plugin_catalog::verify_release_details(&details_envelope, &trust, None, 150)
                .unwrap();
        let joined = verified
            .select_details(&verified_details, "example.echo", "0.1.0", 150)
            .unwrap();
        assert_eq!(joined.distributions.len(), 2);
        assert_eq!(joined.documentation, vec![document("r1")]);
        assert_eq!(
            reopened.latest_details().unwrap().unwrap(),
            details_envelope
        );
        let mut next_details = release_details.clone();
        next_details.documentation.push(document("r2"));
        assert!(
            reopened
                .submit_details("author", &next_details, 109)
                .is_err()
        );
        assert!(
            reopened
                .submit_details_revision("stranger", &next_details, 109)
                .is_err()
        );
        let mut changed_old = next_details.clone();
        changed_old.documentation[0].digest = lenso_plugin_catalog::digest(b"changed");
        assert!(
            reopened
                .submit_details_revision("author", &changed_old, 109)
                .is_err()
        );
        let mut changed_distribution = next_details.clone();
        changed_distribution.distributions.pop();
        assert!(
            reopened
                .submit_details_revision("author", &changed_distribution, 109)
                .is_err()
        );
        let mut invalid_media = next_details.clone();
        invalid_media.documentation[1].media_type = "text/mdx".into();
        assert!(
            reopened
                .submit_details_revision("author", &invalid_media, 109)
                .is_err()
        );
        let mut insecure_url = next_details.clone();
        insecure_url.documentation[1].url = "http://example.test/document.md".into();
        assert!(
            reopened
                .submit_details_revision("author", &insecure_url, 109)
                .is_err()
        );
        let mut oversized = next_details.clone();
        oversized.documentation[1].size = 1024 * 1024 + 1;
        assert!(
            reopened
                .submit_details_revision("author", &oversized, 109)
                .is_err()
        );
        let revision_id = reopened
            .submit_details_revision("author", &next_details, 109)
            .unwrap();
        assert_eq!(
            reopened
                .submit_details_revision("author", &next_details, 109)
                .unwrap(),
            revision_id
        );
        assert!(
            reopened
                .inspect_details_revision("stranger", revision_id)
                .is_err()
        );
        let mut competing = next_details.clone();
        competing.documentation.push(document("r3"));
        assert!(
            reopened
                .submit_details_revision("author", &competing, 109)
                .is_err()
        );
        let (_, revision_digest, state) = reopened
            .inspect_details_revision("author", revision_id)
            .unwrap();
        assert_eq!(state, "awaiting_review");
        assert!(
            reopened
                .approve_details_revision("author", revision_id, &revision_digest, "v1", 110)
                .is_err()
        );
        assert!(
            reopened
                .approve_details_revision("reviewer", revision_id, "wrong", "v1", 110)
                .is_err()
        );
        reopened
            .approve_details_revision("reviewer", revision_id, &revision_digest, "v1", 110)
            .unwrap();
        let reviewed_body: String = reopened
            .connection
            .query_row(
                "SELECT body FROM details_amendments WHERE id=?1",
                [revision_id],
                |row| row.get(0),
            )
            .unwrap();
        reopened
            .connection
            .execute(
                "UPDATE details_amendments SET body='{}' WHERE id=?1",
                [revision_id],
            )
            .unwrap();
        assert!(
            reopened
                .publish_details("reviewer", 1, 111, 200, "test-key", &key)
                .is_err()
        );
        assert_eq!(
            reopened.latest_details().unwrap().unwrap(),
            details_envelope
        );
        reopened
            .connection
            .execute(
                "UPDATE details_amendments SET body=?1 WHERE id=?2",
                params![reviewed_body, revision_id],
            )
            .unwrap();
        let next_envelope = reopened
            .publish_details("reviewer", 1, 111, 200, "test-key", &key)
            .unwrap();
        assert!(
            reopened
                .publish_details("reviewer", 1, 111, 200, "test-key", &key)
                .is_err()
        );
        let verified_next = lenso_plugin_catalog::verify_release_details(
            &next_envelope,
            &trust,
            Some(verified_details.checkpoint()),
            150,
        )
        .unwrap();
        assert_eq!(
            verified
                .select_details(&verified_next, "example.echo", "0.1.0", 150)
                .unwrap()
                .documentation,
            vec![document("r1"), document("r2")]
        );
        assert_eq!(
            reopened
                .inspect_details_revision("author", revision_id)
                .unwrap()
                .2,
            "published"
        );
        let next_revision_id = reopened
            .submit_details_revision("author", &competing, 112)
            .unwrap();
        assert_ne!(next_revision_id, revision_id);
        assert_eq!(
            reopened.inspect_submission("author", id).unwrap().2,
            "published"
        );
        let mut changed = release();
        changed.artifact.digest = lenso_plugin_catalog::digest(b"changed");
        assert!(reopened.insert_verified("author", &changed, 110).is_err());
        drop(reopened);
        let reader = PublishedDirectory::open(&path, "test").unwrap();
        assert_eq!(
            reader.latest_details().unwrap().unwrap(),
            String::from_utf8(next_envelope).unwrap()
        );
    }
}
