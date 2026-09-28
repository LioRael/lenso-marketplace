use super::npm_archive::verify_npm_archive;
use super::*;
use lenso_plugin_catalog::package::{PackageRelease, PackageSnapshot};

impl Directory {
    /// The caller supplies exact local `.tgz` bytes for every signed distribution.
    /// Matching these bytes does not prove that the named registry serves them.
    pub fn submit_package(
        &mut self,
        actor: &str,
        release: &PackageRelease,
        archives: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<i64> {
        release.validate()?;
        ensure!(
            release.availability == Availability::Listed,
            "new package submissions must be listed candidates"
        );
        ensure!(
            archives.len() == release.distributions.len(),
            "one exact npm archive is required for each distribution"
        );
        for distribution in &release.distributions {
            let bytes = archives
                .get(&distribution.id)
                .context("npm archive is missing for distribution")?;
            verify_npm_archive(
                bytes,
                &distribution.package,
                &distribution.version,
                &release.plugin_id,
                &release.version,
                distribution
                    .integrity
                    .as_deref()
                    .context("npm integrity missing")?,
            )?;
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let identity = format!("{}@{}", release.plugin_id, release.version);
        let collision: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM submissions WHERE identity=?1 UNION ALL SELECT 1 FROM linked_cargo_submissions WHERE identity=?1)",
            [&identity],
            |row| row.get(0),
        )?;
        ensure!(
            !collision,
            "a Portable or linked Cargo base Release already owns this identity"
        );
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
        let existing: Option<(i64, String)> = transaction
            .query_row(
                "SELECT id,digest FROM package_submissions WHERE identity=?1",
                [&identity],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((id, previous)) = existing {
            ensure!(
                previous == body_digest,
                "package identity already submitted with different content"
            );
            return Ok(id);
        }
        let count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM package_submissions", [], |row| {
                row.get(0)
            })?;
        ensure!(
            count < i64::try_from(lenso_plugin_catalog::MAX_RELEASES)?,
            "package submission limit reached"
        );
        transaction.execute(
            "INSERT INTO package_submissions(identity,publisher,body,digest,state) VALUES(?1,?2,?3,?4,'awaiting_review')",
            params![identity, release.publisher_id, body, body_digest],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'submit_package',?2,?3)",
            params![actor, identity, i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn inspect_package_submission(
        &self,
        actor: &str,
        submission: i64,
    ) -> Result<(PackageRelease, String, String)> {
        let (body, digest, state, publisher): (String, String, String, String) =
            self.connection.query_row(
                "SELECT body,digest,state,publisher FROM package_submissions WHERE id=?1",
                [submission],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        let release: PackageRelease = serde_json::from_str(&body)?;
        let claims: Vec<String> = self
            .connection
            .prepare("SELECT namespace FROM namespaces WHERE publisher=?1 AND actor=?2")?
            .query_map(params![publisher, actor], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let owner = claims
            .iter()
            .any(|namespace| release.plugin_id.starts_with(&format!("{namespace}.")));
        ensure!(
            owner || self.reviewers.contains(actor),
            "package submission access denied"
        );
        ensure!(
            digest == super::digest(body.as_bytes()),
            "stored package release digest mismatch"
        );
        Ok((release, digest, state))
    }

    pub fn approve_package(
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
            "UPDATE package_submissions SET state='approved',reviewer=?1,policy=?2 WHERE id=?3 AND digest=?4 AND state='awaiting_review'",
            params![actor, policy, submission, expected_digest],
        )?;
        ensure!(
            changed == 1,
            "package submission changed or is not awaiting review"
        );
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'approve_package',?2,?3)",
            params![actor, submission.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn publish_package(
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
            "SELECT COALESCE(MAX(revision),0) FROM package_snapshots",
            [],
            |row| row.get(0),
        )?;
        ensure!(
            current == i64::try_from(expected_revision)?,
            "package revision changed; read publication result"
        );
        let bodies: Vec<(String, String)> = transaction
            .prepare("SELECT body,digest FROM package_submissions WHERE state IN ('approved','published') ORDER BY identity")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let releases = bodies
            .iter()
            .map(|(body, reviewed_digest)| {
                ensure!(
                    digest(body.as_bytes()) == *reviewed_digest,
                    "reviewed package release digest mismatch"
                );
                let release: PackageRelease = serde_json::from_str(body)?;
                release.validate()?;
                Ok(release)
            })
            .collect::<Result<Vec<_>>>()?;
        let revision = current
            .checked_add(1)
            .context("package revision overflow")?;
        let envelope = lenso_plugin_catalog::package::sign(
            &PackageSnapshot::new(
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
            "INSERT INTO package_snapshots VALUES(?1,?2)",
            params![revision, envelope],
        )?;
        transaction.execute(
            "UPDATE package_submissions SET state='published' WHERE state='approved'",
            [],
        )?;
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'publish_package',?2,?3)",
            params![actor, revision.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(envelope)
    }

    pub fn latest_package(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .connection
            .query_row(
                "SELECT envelope FROM package_snapshots ORDER BY revision DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }
}
