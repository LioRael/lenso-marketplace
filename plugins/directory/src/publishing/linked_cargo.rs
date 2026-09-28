use super::*;
use lenso_plugin_catalog::linked_cargo::{LinkedCargoRelease, LinkedCargoSnapshot};
use std::io::Read as _;

const MAX_CRATE_BYTES: usize = 32 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4096;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedCargoCrateIdentity {
    pub package: String,
    pub version: String,
    pub plugin_id: String,
}

/// Inspect exact `.crate` bytes without executing or unpacking them to disk.
/// This is shared by author preparation and Directory admission.
pub fn linked_cargo_crate_identity(bytes: &[u8]) -> Result<LinkedCargoCrateIdentity> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_CRATE_BYTES,
        "crate archive size exceeds limit"
    );
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    let mut manifest = None;
    let mut root = None;
    let mut unpacked = 0u64;
    let mut count = 0usize;
    for entry in archive.entries().context("invalid crate archive")? {
        let mut entry = entry.context("invalid crate archive entry")?;
        count += 1;
        ensure!(
            count <= MAX_ARCHIVE_ENTRIES,
            "crate archive has too many entries"
        );
        unpacked = unpacked
            .checked_add(entry.size())
            .context("crate archive size overflow")?;
        ensure!(
            unpacked <= MAX_UNPACKED_BYTES,
            "crate archive uncompressed size exceeds limit"
        );
        let path = std::str::from_utf8(&entry.path_bytes())?.to_owned();
        let (archive_root, relative_path) = path
            .split_once('/')
            .context("crate archive path is outside its package root")?;
        ensure!(
            !archive_root.is_empty(),
            "crate archive path is outside its package root"
        );
        if let Some(root) = &root {
            ensure!(
                root == archive_root,
                "crate archive has multiple package roots"
            );
        } else {
            root = Some(archive_root.to_owned());
        }
        ensure!(
            !relative_path.contains('\\')
                && relative_path
                    .split('/')
                    .all(|component| !matches!(component, "" | "." | "..")),
            "crate archive path is outside its package root"
        );
        ensure!(
            entry.header().entry_type().is_file(),
            "crate archive contains a non-file entry"
        );
        if relative_path == "Cargo.toml" {
            ensure!(manifest.is_none(), "crate archive has duplicate Cargo.toml");
            ensure!(
                entry.size() <= MAX_MANIFEST_BYTES,
                "crate manifest exceeds size limit"
            );
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            manifest = Some(contents);
        }
    }
    let manifest: toml::Value =
        toml::from_str(&manifest.context("crate archive is missing Cargo.toml")?)
            .context("invalid crate Cargo.toml")?;
    let package = manifest["package"]["name"]
        .as_str()
        .context("crate package name is missing")?
        .to_owned();
    let version = manifest["package"]["version"]
        .as_str()
        .context("crate package version is missing")?
        .to_owned();
    let plugin_id = manifest["package"]["metadata"]["lenso"]["plugin-id"]
        .as_str()
        .context("crate Lenso Plugin ID is missing")?
        .to_owned();
    ensure!(
        root.as_deref() == Some(format!("{package}-{version}").as_str()),
        "crate archive root does not match package and version"
    );
    Ok(LinkedCargoCrateIdentity {
        package,
        version,
        plugin_id,
    })
}

fn verify_crate_manifest(release: &LinkedCargoRelease, bytes: &[u8]) -> Result<()> {
    let identity = linked_cargo_crate_identity(bytes)?;
    ensure!(
        identity.package == release.package,
        "crate package name does not match release"
    );
    ensure!(
        identity.version == release.version,
        "crate package version does not match release"
    );
    ensure!(
        identity.plugin_id == release.plugin_id,
        "crate Plugin ID does not match release"
    );
    Ok(())
}

impl Directory {
    /// Reviewable source submission; archive bytes are checked before any write.
    pub fn submit_linked_cargo(
        &mut self,
        actor: &str,
        release: &LinkedCargoRelease,
        crate_archive: &[u8],
        now: u64,
    ) -> Result<i64> {
        release.validate()?;
        ensure!(
            release.availability == Availability::Listed,
            "new linked Cargo submissions must be listed candidates"
        );
        ensure!(
            !crate_archive.is_empty() && crate_archive.len() <= MAX_CRATE_BYTES,
            "crate archive size exceeds limit"
        );
        ensure!(
            digest(crate_archive) == release.crate_digest,
            "crate archive digest does not match linked Cargo release"
        );
        verify_crate_manifest(release, crate_archive)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let identity = format!("{}@{}", release.plugin_id, release.version);
        super::release_content::ensure_no_content_only_owner(&transaction, &identity)?;
        let existing_portable: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM submissions WHERE identity=?1)",
            [&identity],
            |row| row.get(0),
        )?;
        ensure!(
            !existing_portable,
            "a portable base Release already owns this identity; use release details for its linked distribution"
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
        let owns_namespace: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM namespaces WHERE publisher=?1 AND actor=?2 AND (?3 LIKE namespace || '.%'))",
            params![release.publisher_id, actor, release.plugin_id],
            |row| row.get(0),
        )?;
        ensure!(owns_namespace, "publisher does not own this namespace");
        let body = serde_json::to_string(release)?;
        let body_digest = digest(body.as_bytes());
        let existing: Option<(i64, String)> = transaction
            .query_row(
                "SELECT id,digest FROM linked_cargo_submissions WHERE identity=?1",
                [&identity],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((id, previous)) = existing {
            ensure!(
                previous == body_digest,
                "linked Cargo identity already submitted with different content"
            );
            return Ok(id);
        }
        let count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM linked_cargo_submissions", [], |row| {
                row.get(0)
            })?;
        ensure!(
            count < i64::try_from(lenso_plugin_catalog::MAX_RELEASES)?,
            "linked Cargo submission limit reached"
        );
        transaction.execute(
            "INSERT INTO linked_cargo_submissions(identity,publisher,body,digest,state) VALUES(?1,?2,?3,?4,'awaiting_review')",
            params![identity, release.publisher_id, body, body_digest],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'submit_linked_cargo',?2,?3)",
            params![actor, identity, i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn inspect_linked_cargo_submission(
        &self,
        actor: &str,
        submission: i64,
    ) -> Result<(LinkedCargoRelease, String, String)> {
        let (body, digest, state, publisher): (String, String, String, String) =
            self.connection.query_row(
                "SELECT body,digest,state,publisher FROM linked_cargo_submissions WHERE id=?1",
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
            "linked Cargo submission access denied"
        );
        Ok((serde_json::from_str(&body)?, digest, state))
    }

    pub fn approve_linked_cargo(
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
            "UPDATE linked_cargo_submissions SET state='approved',reviewer=?1,policy=?2 WHERE id=?3 AND digest=?4 AND state='awaiting_review'",
            params![actor, policy, submission, expected_digest],
        )?;
        ensure!(
            changed == 1,
            "linked Cargo submission changed or is not awaiting review"
        );
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'approve_linked_cargo',?2,?3)",
            params![actor, submission.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn submit_linked_cargo_document_revision(
        &mut self,
        actor: &str,
        release: &LinkedCargoRelease,
        now: u64,
    ) -> Result<i64> {
        release.validate()?;
        let identity = format!("{}@{}", release.plugin_id, release.version);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (base_body, base_digest, publisher, state): (String, String, String, String) =
            transaction.query_row(
                "SELECT body,digest,publisher,state FROM linked_cargo_submissions WHERE identity=?1",
                [&identity],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        ensure!(
            digest(base_body.as_bytes()) == base_digest,
            "stored linked Cargo release digest mismatch"
        );
        ensure!(
            state == "published",
            "linked Cargo release is not published"
        );
        let owns_namespace: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM namespaces WHERE publisher=?1 AND actor=?2)",
            params![publisher, actor],
            |row| row.get(0),
        )?;
        ensure!(owns_namespace, "publisher does not own this namespace");
        let latest: Option<(i64, String, String, String)> = transaction
            .query_row(
                "SELECT id,body,digest,state FROM linked_cargo_amendments WHERE identity=?1 ORDER BY id DESC LIMIT 1",
                [&identity],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let body = serde_json::to_string(release)?;
        if let Some((id, previous, previous_digest, _)) = &latest {
            ensure!(
                digest(previous.as_bytes()) == *previous_digest,
                "stored linked Cargo documentation revision digest mismatch"
            );
            if previous == &body {
                return Ok(*id);
            }
        }
        ensure!(
            latest
                .as_ref()
                .is_none_or(|(_, _, _, state)| state == "published"),
            "another linked Cargo documentation revision is awaiting publication"
        );
        let previous_body = latest
            .as_ref()
            .map_or(base_body.as_str(), |(_, body, _, _)| body.as_str());
        let previous: LinkedCargoRelease = serde_json::from_str(previous_body)?;
        ensure_additive_linked_documentation(&previous, release)?;
        let body_digest = digest(body.as_bytes());
        transaction.execute(
            "INSERT INTO linked_cargo_amendments(identity,publisher,body,digest,base_digest,state) VALUES(?1,?2,?3,?4,?5,'awaiting_review')",
            params![identity, publisher, body, body_digest, digest(previous_body.as_bytes())],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'submit_linked_cargo_document_revision',?2,?3)",
            params![actor, id.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn inspect_linked_cargo_document_revision(
        &self,
        actor: &str,
        amendment: i64,
    ) -> Result<(LinkedCargoRelease, String, String)> {
        let (body, digest, state, publisher): (String, String, String, String) =
            self.connection.query_row(
                "SELECT body,digest,state,publisher FROM linked_cargo_amendments WHERE id=?1",
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
            "linked Cargo documentation revision access denied"
        );
        Ok((serde_json::from_str(&body)?, digest, state))
    }

    pub fn approve_linked_cargo_document_revision(
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
            "UPDATE linked_cargo_amendments SET state='approved',reviewer=?1,policy=?2 WHERE id=?3 AND digest=?4 AND state='awaiting_review'",
            params![actor, policy, amendment, expected_digest],
        )?;
        ensure!(
            changed == 1,
            "linked Cargo documentation revision changed or is not awaiting review"
        );
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'approve_linked_cargo_document_revision',?2,?3)",
            params![actor, amendment.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn publish_linked_cargo(
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
            "SELECT COALESCE(MAX(revision),0) FROM linked_cargo_snapshots",
            [],
            |row| row.get(0),
        )?;
        ensure!(
            current == i64::try_from(expected_revision)?,
            "linked Cargo revision changed; read publication result"
        );
        let bodies: Vec<(String, String)> = transaction.prepare(
            "SELECT body,digest FROM linked_cargo_submissions WHERE state IN ('approved','published') ORDER BY identity"
        )?.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut releases = BTreeMap::<String, (LinkedCargoRelease, String)>::new();
        for (body, reviewed_digest) in &bodies {
            ensure!(
                digest(body.as_bytes()) == *reviewed_digest,
                "reviewed linked Cargo release digest mismatch"
            );
            let release: LinkedCargoRelease = serde_json::from_str(body)?;
            let identity = format!("{}@{}", release.plugin_id, release.version);
            ensure!(
                releases
                    .insert(identity, (release, reviewed_digest.clone()))
                    .is_none(),
                "duplicate linked Cargo release identity"
            );
        }
        let amendments: Vec<(String, String, String, String, String)> = transaction
            .prepare("SELECT identity,body,digest,base_digest,state FROM linked_cargo_amendments WHERE state IN ('approved','published') ORDER BY id")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut approved = BTreeSet::new();
        for (identity, body, reviewed_digest, base_digest, state) in amendments {
            ensure!(
                digest(body.as_bytes()) == reviewed_digest,
                "reviewed linked Cargo documentation revision digest mismatch"
            );
            let (previous, previous_digest) = releases
                .get(&identity)
                .context("linked Cargo documentation revision has no published base")?;
            ensure!(
                previous_digest == &base_digest,
                "linked Cargo documentation revision base changed"
            );
            let release: LinkedCargoRelease = serde_json::from_str(&body)?;
            release.validate()?;
            ensure_additive_linked_documentation(previous, &release)?;
            if state == "approved" {
                ensure!(
                    approved.insert(identity.clone()),
                    "multiple approved linked Cargo documentation revisions"
                );
            }
            releases.insert(identity, (release, reviewed_digest));
        }
        let releases: Vec<LinkedCargoRelease> =
            releases.into_values().map(|(release, _)| release).collect();
        let revision = current
            .checked_add(1)
            .context("linked Cargo revision overflow")?;
        let envelope = lenso_plugin_catalog::linked_cargo::sign(
            &LinkedCargoSnapshot::new(
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
            "INSERT INTO linked_cargo_snapshots VALUES(?1,?2)",
            params![revision, envelope],
        )?;
        transaction.execute(
            "UPDATE linked_cargo_submissions SET state='published' WHERE state='approved'",
            [],
        )?;
        transaction.execute(
            "UPDATE linked_cargo_amendments SET state='published' WHERE state='approved'",
            [],
        )?;
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'publish_linked_cargo',?2,?3)",
            params![actor, revision.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(envelope)
    }

    pub fn latest_linked_cargo(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .connection
            .query_row(
                "SELECT envelope FROM linked_cargo_snapshots ORDER BY revision DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }
}

fn ensure_additive_linked_documentation(
    previous: &LinkedCargoRelease,
    next: &LinkedCargoRelease,
) -> Result<()> {
    let mut expected = previous.clone();
    expected.documentation = next.documentation.clone();
    ensure!(
        expected == *next,
        "linked Cargo documentation revision changed the release"
    );
    ensure!(
        next.documentation.len() > previous.documentation.len(),
        "linked Cargo documentation revision must add a new document identity"
    );
    for old in &previous.documentation {
        ensure!(
            next.documentation.iter().any(|document| document == old),
            "linked Cargo documentation revision removed or changed a published document"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use lenso_plugin_catalog::{
        Availability, Documentation, Trust,
        linked_cargo::{LinkedCargoIntegration, verify},
    };

    use super::*;

    fn crate_archive(package: &str, version: &str, plugin_id: &str) -> Vec<u8> {
        let manifest = format!(
            "[package]\nname = \"{package}\"\nversion = \"{version}\"\n[package.metadata.lenso]\nplugin-id = \"{plugin_id}\"\n"
        );
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(
                &mut header,
                format!("{package}-{version}/Cargo.toml"),
                manifest.as_bytes(),
            )
            .unwrap();
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn crate_archive_with_windows_escape() -> Vec<u8> {
        let manifest = "[package]\nname = \"example-web-plugin\"\nversion = \"0.4.5\"\n[package.metadata.lenso]\nplugin-id = \"example.web\"\n";
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (path, contents) in [
            ("example-web-plugin-0.4.5/Cargo.toml", manifest.as_bytes()),
            (
                "example-web-plugin-0.4.5/src\\..\\escape.rs",
                b"escape".as_slice(),
            ),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, path, contents).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn release(crate_archive: &[u8]) -> LinkedCargoRelease {
        LinkedCargoRelease {
            plugin_id: "example.web".into(),
            version: "0.4.5".into(),
            publisher_id: "publisher".into(),
            title: "Web".into(),
            summary: "Linked Web Plugin".into(),
            source_url: "https://github.com/example/web".into(),
            source_revision: "a".repeat(40),
            license: "MIT".into(),
            package: "example-web-plugin".into(),
            registry_url: "https://crates.io".into(),
            crate_digest: digest(crate_archive),
            integration: LinkedCargoIntegration::LinkedPlugin,
            targets: vec!["aarch64-apple-darwin".into()],
            availability: Availability::Listed,
            documentation: Vec::new(),
        }
    }

    fn document(revision: &str) -> Documentation {
        Documentation {
            id: "getting-started".into(),
            revision: revision.into(),
            language: "en".into(),
            topic: "start".into(),
            target: None,
            url: format!("https://example.test/docs/example.web/0.4.5/{revision}.md"),
            digest: digest(revision.as_bytes()),
            size: revision.len() as u64,
            media_type: "text/markdown".into(),
        }
    }

    #[test]
    fn source_only_release_requires_review_and_preserves_old_catalog() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("directory.db");
        let key = SigningKey::from_bytes(&[19; 32]);
        let trust = Trust {
            catalog_id: "catalog".into(),
            keys: BTreeMap::from([("key".into(), key.verifying_key())]),
        };
        let mut directory =
            Directory::open(&path, "catalog", BTreeSet::from(["reviewer".into()])).unwrap();
        directory
            .claim_namespace("reviewer", "example", "publisher", "author", 100)
            .unwrap();
        let archive = crate_archive("example-web-plugin", "0.4.5", "example.web");
        assert!(
            directory
                .submit_linked_cargo("author", &release(&archive), b"wrong", 101)
                .is_err()
        );
        assert!(
            directory
                .submit_linked_cargo("stranger", &release(&archive), &archive, 101)
                .is_err()
        );
        let mut source_release = release(&archive);
        source_release.documentation.push(document("r1"));
        let id = directory
            .submit_linked_cargo("author", &source_release, &archive, 101)
            .unwrap();
        let (_, digest, state) = directory
            .inspect_linked_cargo_submission("author", id)
            .unwrap();
        assert_eq!(state, "awaiting_review");
        assert!(
            directory
                .publish_linked_cargo("author", 0, 102, 200, "key", &key)
                .is_err()
        );
        directory
            .approve_linked_cargo("reviewer", id, &digest, "v1", 103)
            .unwrap();
        let envelope = directory
            .publish_linked_cargo("reviewer", 0, 104, 200, "key", &key)
            .unwrap();
        let verified = verify(&envelope, &trust, None, 150).unwrap();
        assert_eq!(
            verified
                .select("example.web", "0.4.5", 150)
                .unwrap()
                .documentation,
            vec![document("r1")]
        );
        assert_eq!(
            directory.latest_linked_cargo().unwrap(),
            Some(envelope.clone())
        );
        assert!(directory.latest().unwrap().is_none());
        assert!(directory.latest_details().unwrap().is_none());
        let reader = PublishedDirectory::open(&path, "catalog").unwrap();
        assert_eq!(
            reader.latest_linked_cargo().unwrap(),
            Some(String::from_utf8(envelope).unwrap())
        );
    }

    #[test]
    fn linked_document_revision_is_additive_and_keeps_previous_signature() {
        let temp = tempfile::tempdir().unwrap();
        let key = SigningKey::from_bytes(&[20; 32]);
        let trust = Trust {
            catalog_id: "catalog".into(),
            keys: BTreeMap::from([("key".into(), key.verifying_key())]),
        };
        let mut directory = Directory::open(
            &temp.path().join("directory.db"),
            "catalog",
            BTreeSet::from(["reviewer".into()]),
        )
        .unwrap();
        directory
            .claim_namespace("reviewer", "example", "publisher", "author", 100)
            .unwrap();
        let archive = crate_archive("example-web-plugin", "0.4.5", "example.web");
        let mut source_release = release(&archive);
        source_release.documentation.push(document("r1"));
        let id = directory
            .submit_linked_cargo("author", &source_release, &archive, 101)
            .unwrap();
        let (_, proposal_digest, _) = directory
            .inspect_linked_cargo_submission("author", id)
            .unwrap();
        directory
            .approve_linked_cargo("reviewer", id, &proposal_digest, "v1", 102)
            .unwrap();
        let mut premature = source_release.clone();
        premature.documentation.push(document("r2"));
        assert!(
            directory
                .submit_linked_cargo_document_revision("author", &premature, 102)
                .is_err()
        );
        let first = directory
            .publish_linked_cargo("reviewer", 0, 103, 200, "key", &key)
            .unwrap();

        let mut next = source_release.clone();
        next.documentation.push(document("r2"));
        assert!(
            directory
                .submit_linked_cargo_document_revision("stranger", &next, 104)
                .is_err()
        );
        let mut changed = next.clone();
        changed.documentation[0].digest = digest(b"changed");
        assert!(
            directory
                .submit_linked_cargo_document_revision("author", &changed, 104)
                .is_err()
        );
        changed = next.clone();
        changed.crate_digest = digest(b"different crate");
        assert!(
            directory
                .submit_linked_cargo_document_revision("author", &changed, 104)
                .is_err()
        );
        let amendment = directory
            .submit_linked_cargo_document_revision("author", &next, 104)
            .unwrap();
        assert_eq!(
            directory
                .submit_linked_cargo_document_revision("author", &next, 104)
                .unwrap(),
            amendment
        );
        let mut competing = next.clone();
        competing.documentation.push(document("r3"));
        assert!(
            directory
                .submit_linked_cargo_document_revision("author", &competing, 104)
                .is_err()
        );
        let (_, amendment_digest, state) = directory
            .inspect_linked_cargo_document_revision("author", amendment)
            .unwrap();
        assert_eq!(state, "awaiting_review");
        assert!(
            directory
                .approve_linked_cargo_document_revision("reviewer", amendment, "wrong", "v1", 105)
                .is_err()
        );
        directory
            .approve_linked_cargo_document_revision(
                "reviewer",
                amendment,
                &amendment_digest,
                "v1",
                105,
            )
            .unwrap();
        assert!(
            directory
                .publish_linked_cargo("reviewer", 0, 106, 200, "key", &key)
                .is_err()
        );
        let reviewed_body: String = directory
            .connection
            .query_row(
                "SELECT body FROM linked_cargo_amendments WHERE id=?1",
                [amendment],
                |row| row.get(0),
            )
            .unwrap();
        directory
            .connection
            .execute(
                "UPDATE linked_cargo_amendments SET body='{}' WHERE id=?1",
                [amendment],
            )
            .unwrap();
        assert!(
            directory
                .publish_linked_cargo("reviewer", 1, 106, 200, "key", &key)
                .is_err()
        );
        assert_eq!(
            directory.latest_linked_cargo().unwrap(),
            Some(first.clone())
        );
        directory
            .connection
            .execute(
                "UPDATE linked_cargo_amendments SET body=?1 WHERE id=?2",
                params![reviewed_body, amendment],
            )
            .unwrap();
        let second = directory
            .publish_linked_cargo("reviewer", 1, 106, 200, "key", &key)
            .unwrap();
        assert_eq!(
            directory.latest_linked_cargo().unwrap(),
            Some(second.clone())
        );
        let old = verify(&first, &trust, None, 150).unwrap();
        assert_eq!(
            old.select("example.web", "0.4.5", 150)
                .unwrap()
                .documentation,
            vec![document("r1")]
        );
        let updated = verify(&second, &trust, Some(old.checkpoint()), 150).unwrap();
        assert_eq!(
            updated
                .select("example.web", "0.4.5", 150)
                .unwrap()
                .documentation,
            vec![document("r1"), document("r2")]
        );
        assert_eq!(
            directory
                .inspect_linked_cargo_document_revision("author", amendment)
                .unwrap()
                .2,
            "published"
        );
        directory
            .submit_linked_cargo_document_revision("author", &competing, 107)
            .unwrap();
        assert!(directory.latest().unwrap().is_none());
        assert!(directory.latest_details().unwrap().is_none());
    }

    #[test]
    fn rejects_forged_or_non_crate_archives_before_write() {
        let temp = tempfile::tempdir().unwrap();
        let mut directory = Directory::open(
            &temp.path().join("directory.db"),
            "catalog",
            BTreeSet::from(["reviewer".into()]),
        )
        .unwrap();
        directory
            .claim_namespace("reviewer", "example", "publisher", "author", 100)
            .unwrap();
        let wrong_plugin = crate_archive("example-web-plugin", "0.4.5", "example.other");
        assert!(
            directory
                .submit_linked_cargo("author", &release(&wrong_plugin), &wrong_plugin, 101)
                .is_err()
        );
        let wrong_version = crate_archive("example-web-plugin", "0.4.4", "example.web");
        assert!(
            directory
                .submit_linked_cargo("author", &release(&wrong_version), &wrong_version, 101)
                .is_err()
        );
        assert!(
            directory
                .submit_linked_cargo("author", &release(b"not a crate"), b"not a crate", 101)
                .is_err()
        );
        let windows_escape = crate_archive_with_windows_escape();
        assert!(
            directory
                .submit_linked_cargo("author", &release(&windows_escape), &windows_escape, 101)
                .is_err()
        );
        let count: i64 = directory
            .connection
            .query_row("SELECT COUNT(*) FROM linked_cargo_submissions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    #[ignore = "set LENSO_REAL_CRATE_ARCHIVE to the exact cached registry .crate"]
    fn accepts_real_published_web_ingress_crate() {
        let archive = std::fs::read(std::env::var("LENSO_REAL_CRATE_ARCHIVE").unwrap()).unwrap();
        let expected = "sha256:76cf4784b0706b269b0a3d475aafca181853c1c1085f0bc3715d93facbedad36";
        assert_eq!(digest(&archive), expected);
        let mut release = release(&archive);
        release.plugin_id = "lenso.web-ingress".into();
        release.package = "lenso-web-ingress-plugin".into();
        release.integration = LinkedCargoIntegration::HostProvided;
        release.source_revision = "0e93f1149ac1b0905a51d76692d369a028f3a532".into();
        let temp = tempfile::tempdir().unwrap();
        let mut directory = Directory::open(
            &temp.path().join("directory.db"),
            "catalog",
            BTreeSet::from(["reviewer".into()]),
        )
        .unwrap();
        directory
            .claim_namespace("reviewer", "lenso", "publisher", "author", 100)
            .unwrap();
        directory
            .submit_linked_cargo("author", &release, &archive, 101)
            .unwrap();
    }
}
