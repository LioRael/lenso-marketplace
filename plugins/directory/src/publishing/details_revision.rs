//! Append-only documentation metadata amendments for an exact published release.
//! The initial release-details submission and every signed snapshot remain intact.
use super::{Directory, Release, ReleaseDetails, digest};
use anyhow::{Result, ensure};
use rusqlite::{OptionalExtension as _, TransactionBehavior, params};

impl Directory {
    /// Stage a complete next view, never a partial patch or in-place rewrite.
    pub fn submit_details_revision(
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
        let (base_body, base_digest, publisher, base_state): (String, String, String, String) =
            transaction.query_row(
                "SELECT body,digest,publisher,state FROM details_submissions WHERE identity=?1",
                [&identity],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        ensure!(
            digest(base_body.as_bytes()) == base_digest,
            "stored release details digest mismatch"
        );
        ensure!(
            base_state == "published",
            "release details are not published"
        );
        let owns_namespace: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM namespaces WHERE publisher=?1 AND actor=?2)",
            params![publisher, actor],
            |row| row.get(0),
        )?;
        ensure!(owns_namespace, "publisher does not own this namespace");
        let latest: Option<(i64, String, String, String)> = transaction
            .query_row(
                "SELECT id,body,digest,state FROM details_amendments WHERE identity=?1 ORDER BY id DESC LIMIT 1",
                [&identity],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let body = serde_json::to_string(details)?;
        if let Some((id, previous, previous_digest, _)) = &latest {
            ensure!(
                digest(previous.as_bytes()) == *previous_digest,
                "stored documentation revision digest mismatch"
            );
            if previous == &body {
                return Ok(*id);
            }
        }
        ensure!(
            latest
                .as_ref()
                .is_none_or(|(_, _, _, state)| state == "published"),
            "another documentation revision is awaiting publication"
        );
        let previous_body = latest
            .as_ref()
            .map_or(base_body.as_str(), |(_, body, _, _)| body.as_str());
        let previous: ReleaseDetails = serde_json::from_str(previous_body)?;
        ensure_additive_documents(&previous, details)?;
        let base_release_body: String = transaction.query_row(
            "SELECT body FROM submissions WHERE identity=?1 AND state='published'",
            [&identity],
            |row| row.get(0),
        )?;
        let base_release: Release = serde_json::from_str(&base_release_body)?;
        details.validate_against(&base_release)?;
        let body_digest = digest(body.as_bytes());
        transaction.execute(
            "INSERT INTO details_amendments(identity,publisher,body,digest,base_digest,state) VALUES(?1,?2,?3,?4,?5,'awaiting_review')",
            params![identity, publisher, body, body_digest, digest(previous_body.as_bytes())],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'submit_details_revision',?2,?3)",
            params![actor, id.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn inspect_details_revision(
        &self,
        actor: &str,
        amendment: i64,
    ) -> Result<(ReleaseDetails, String, String)> {
        let (body, digest, state, publisher): (String, String, String, String) =
            self.connection.query_row(
                "SELECT body,digest,state,publisher FROM details_amendments WHERE id=?1",
                [amendment],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        let owner: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM namespaces WHERE publisher=?1 AND actor=?2)",
            params![publisher, actor],
            |row| row.get(0),
        )?;
        ensure!(
            owner || self.reviewers.contains(actor),
            "revision access denied"
        );
        Ok((serde_json::from_str(&body)?, digest, state))
    }

    pub fn approve_details_revision(
        &mut self,
        actor: &str,
        amendment: i64,
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
            "UPDATE details_amendments SET state='approved',reviewer=?1,policy=?2 WHERE id=?3 AND digest=?4 AND state='awaiting_review'",
            params![actor, policy, amendment, expected_digest],
        )?;
        ensure!(changed == 1, "revision changed or is not awaiting review");
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'approve_details_revision',?2,?3)",
            params![actor, amendment.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

pub(super) fn ensure_additive_documents(
    previous: &ReleaseDetails,
    next: &ReleaseDetails,
) -> Result<()> {
    ensure!(
        previous.plugin_id == next.plugin_id
            && previous.version == next.version
            && previous.base_release_identity == next.base_release_identity
            && previous.distributions == next.distributions,
        "documentation revision changed immutable release details"
    );
    ensure!(
        next.documentation.len() > previous.documentation.len(),
        "documentation revision must add a new document identity"
    );
    for old in &previous.documentation {
        ensure!(
            next.documentation
                .iter()
                .any(|new| new.id == old.id && new.revision == old.revision && new == old),
            "documentation revision removed or changed a published document"
        );
    }
    Ok(())
}
