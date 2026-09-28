//! Curated publication of optional content for an already published release.
//! The v1 base release bytes are never amended by this channel.

use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer as _, VerifyingKey};
use lenso_plugin_catalog::{Envelope, Trust, linked_cargo::LinkedCargoRelease};
use serde::{Deserialize, Serialize};
use std::io::Read as _;

const SCHEMA: &str = "lenso.marketplace.release-content.v2";
const SIGNATURE_CONTEXT: &[u8] = b"lenso.marketplace.release-content.v2\0";
const MAX_CONTENT_BYTES: u64 = 16 * 1024 * 1024;
// Match the exact source-App adoption bounds, so reviewed content is adoptable.
const MAX_UNPACKED_BYTES: u64 = 32 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 512;
const MAX_TAR_STREAM_BYTES: u64 = 34 * 1024 * 1024;
const ATTRIBUTION: &str = ".lenso-release-content.json";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseKind {
    Portable,
    LinkedCargo,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    EditableTemplate,
    DevelopmentExtension,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub id: String,
    pub kind: ContentKind,
    pub url: String,
    pub digest: String,
    pub size: u64,
}

impl Content {
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty()
                && self.id.len() <= 128
                && self
                    .id
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_lowercase())
                && self.id.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'-' | b'_')
                }),
            "invalid release content ID"
        );
        let url = url::Url::parse(&self.url)?;
        ensure!(
            self.url.len() <= 2048
                && url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none(),
            "expected credential-free HTTPS URL"
        );
        valid_digest(&self.digest)?;
        ensure!(
            self.size > 0 && self.size <= MAX_CONTENT_BYTES,
            "release content size exceeds bounds"
        );
        Ok(())
    }

    fn verify_archive(&self, bytes: &[u8], plugin_id: &str, version: &str) -> Result<()> {
        self.validate()?;
        ensure!(
            u64::try_from(bytes.len())? == self.size && digest(bytes) == self.digest,
            "release content size or digest mismatch"
        );
        let decoder = flate2::read::MultiGzDecoder::new(bytes);
        let mut archive = tar::Archive::new(decoder.take(MAX_TAR_STREAM_BYTES + 1));
        let mut count = 0usize;
        let mut unpacked = 0u64;
        let mut paths = BTreeSet::new();
        let mut source = super::release_content_source::SourceProject::default();
        for entry in archive
            .entries()
            .context("invalid release content archive")?
            // Never let tar pre-read GNU/PAX metadata into an unbounded Vec.
            .raw(true)
        {
            let mut entry = entry.context("invalid release content archive entry")?;
            count += 1;
            ensure!(
                count <= MAX_ARCHIVE_ENTRIES,
                "release content has too many entries"
            );
            ensure!(
                entry.header().entry_type().is_file(),
                "release content contains a non-file entry"
            );
            ensure!(
                entry.size() <= MAX_FILE_BYTES,
                "release content file exceeds size limit"
            );
            unpacked = unpacked
                .checked_add(entry.size())
                .context("release content size overflow")?;
            ensure!(
                unpacked <= MAX_UNPACKED_BYTES,
                "release content expands past size limit"
            );
            let path = std::str::from_utf8(&entry.path_bytes())?.to_owned();
            ensure!(
                path.len() <= 1024
                    && !path.chars().any(char::is_control)
                    && !path.contains('\\')
                    && !path.is_empty()
                    && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
                    && Path::new(&path)
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_)))
                    && path != ATTRIBUTION,
                "release content path is unsafe"
            );
            ensure!(paths.insert(path.clone()), "duplicate release content path");
            let copied = if self.kind == ContentKind::DevelopmentExtension {
                source.read_file(&path, &mut entry)?
            } else {
                std::io::copy(&mut entry, &mut std::io::sink())?
            };
            ensure!(copied == entry.size(), "release content entry is truncated");
        }
        // Tar may stop at its end marker. Include padding, metadata and all
        // concatenated gzip members in the decompressed-stream bound.
        let mut stream = archive.into_inner();
        std::io::copy(&mut stream, &mut std::io::sink())?;
        ensure!(
            stream.limit() > 0,
            "release content tar stream exceeds size limit"
        );
        ensure!(count > 0, "release content archive is empty");
        if self.kind == ContentKind::DevelopmentExtension {
            source.validate(plugin_id, version)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseContent {
    pub plugin_id: String,
    pub version: String,
    pub base_kind: BaseKind,
    pub base_release_identity: String,
    pub content: Vec<Content>,
}

impl ReleaseContent {
    fn validate(&self) -> Result<()> {
        lenso_plugin_catalog::identity::validate_plugin_id_v1(&self.plugin_id)?;
        lenso_plugin_catalog::identity::validate_release_version(&self.version)?;
        lenso_plugin_catalog::bounded_text(&self.version, 128)?;
        valid_digest(&self.base_release_identity)?;
        ensure!(
            !self.content.is_empty() && self.content.len() <= 32,
            "release content needs one to 32 entries"
        );
        let mut ids = BTreeSet::new();
        for content in &self.content {
            content.validate()?;
            ensure!(
                ids.insert(&content.id),
                "duplicate release content identity"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema: String,
    pub catalog_id: String,
    pub revision: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    pub releases: Vec<ReleaseContent>,
}

impl Snapshot {
    fn validate(&self) -> Result<()> {
        ensure!(self.schema == SCHEMA, "unsupported release content schema");
        lenso_plugin_catalog::bounded_text(&self.catalog_id, 128)?;
        ensure!(
            self.revision > 0
                && self.revision <= 9_007_199_254_740_991
                && self.expires_at > self.issued_at
                && self.expires_at <= 9_007_199_254_740_991
                && self.expires_at - self.issued_at <= 7 * 24 * 60 * 60,
            "invalid release content revision or validity window"
        );
        ensure!(
            self.releases.len() <= lenso_plugin_catalog::MAX_RELEASES,
            "too many release content entries"
        );
        let mut identities = BTreeSet::new();
        for release in &self.releases {
            release.validate()?;
            ensure!(
                identities.insert((&release.plugin_id, &release.version)),
                "duplicate release content identity"
            );
        }
        Ok(())
    }
}

fn valid_digest(value: &str) -> Result<()> {
    let hex = value
        .strip_prefix("sha256:")
        .context("unsupported digest")?;
    ensure!(
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid SHA-256 digest"
    );
    Ok(())
}

pub fn linked_identity(release: &LinkedCargoRelease) -> Result<String> {
    release.validate()?;
    Ok(digest(&serde_json::to_vec(&(
        &release.plugin_id,
        &release.version,
        &release.publisher_id,
        &release.title,
        &release.summary,
        &release.source_url,
        &release.source_revision,
        &release.license,
        &release.package,
        &release.registry_url,
        &release.crate_digest,
        release.integration,
        &release.targets,
    ))?))
}

fn base_publisher(connection: &Connection, release: &ReleaseContent) -> Result<String> {
    let identity = format!("{}@{}", release.plugin_id, release.version);
    let (table, label) = match release.base_kind {
        BaseKind::Portable => ("submissions", "portable"),
        BaseKind::LinkedCargo => ("linked_cargo_submissions", "linked Cargo"),
    };
    let query = format!("SELECT body,publisher,state FROM {table} WHERE identity=?1");
    let (body, publisher, state): (String, String, String) = connection
        .query_row(&query, [&identity], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .with_context(|| format!("{label} base release is not submitted"))?;
    ensure!(
        state == "published",
        "release content requires a published base release"
    );
    let actual = match release.base_kind {
        BaseKind::Portable => {
            serde_json::from_str::<lenso_plugin_catalog::Release>(&body)?.immutable_identity()?
        }
        BaseKind::LinkedCargo => {
            linked_identity(&serde_json::from_str::<LinkedCargoRelease>(&body)?)?
        }
    };
    ensure!(
        actual == release.base_release_identity,
        "release content base identity mismatch"
    );
    Ok(publisher)
}

fn signing_bytes(key_id: &str, payload: &[u8]) -> Vec<u8> {
    let mut bytes = SIGNATURE_CONTEXT.to_vec();
    bytes.extend_from_slice(key_id.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(payload);
    bytes
}

pub fn sign(snapshot: &Snapshot, key_id: &str, key: &SigningKey) -> Result<Vec<u8>> {
    snapshot.validate()?;
    lenso_plugin_catalog::bounded_text(key_id, 128)?;
    let payload = serde_json::to_vec(snapshot)?;
    let envelope = Envelope {
        key_id: key_id.into(),
        signature_base64: STANDARD.encode(key.sign(&signing_bytes(key_id, &payload)).to_bytes()),
        payload_base64: STANDARD.encode(payload),
    };
    let bytes = serde_json::to_vec(&envelope)?;
    ensure!(
        bytes.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
        "release content exceeds size limit"
    );
    Ok(bytes)
}

pub fn verify(bytes: &[u8], trust: &Trust, now: u64) -> Result<Snapshot> {
    ensure!(
        bytes.len() <= lenso_plugin_catalog::MAX_ENVELOPE_BYTES,
        "release content exceeds size limit"
    );
    let envelope: Envelope = serde_json::from_slice(bytes)?;
    let key: &VerifyingKey = trust
        .keys
        .get(&envelope.key_id)
        .context("unknown signing key")?;
    let payload = STANDARD.decode(&envelope.payload_base64)?;
    let signature = Signature::from_slice(&STANDARD.decode(&envelope.signature_base64)?)?;
    key.verify_strict(&signing_bytes(&envelope.key_id, &payload), &signature)?;
    let snapshot: Snapshot = serde_json::from_slice(&payload)?;
    snapshot.validate()?;
    ensure!(
        snapshot.catalog_id == trust.catalog_id,
        "unexpected catalog identity"
    );
    ensure!(
        now >= snapshot.issued_at && now < snapshot.expires_at,
        "release content is expired or not yet valid"
    );
    Ok(snapshot)
}

impl Directory {
    pub fn submit_release_content(
        &mut self,
        actor: &str,
        release: &ReleaseContent,
        archives: &[&[u8]],
        now: u64,
    ) -> Result<i64> {
        release.validate()?;
        ensure!(
            archives.len() == release.content.len(),
            "one archive is required for each content entry"
        );
        for (content, archive) in release.content.iter().zip(archives) {
            content.verify_archive(archive, &release.plugin_id, &release.version)?;
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let publisher = base_publisher(&transaction, release)?;
        let owns_namespace: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM namespaces WHERE publisher=?1 AND actor=?2 AND (?3 LIKE namespace || '.%'))",
            params![publisher, actor, release.plugin_id], |row| row.get(0)
        )?;
        ensure!(owns_namespace, "publisher does not own this namespace");
        let identity = format!("{}@{}", release.plugin_id, release.version);
        let body = serde_json::to_string(release)?;
        let body_digest = digest(body.as_bytes());
        let existing: Option<(i64, String)> = transaction
            .query_row(
                "SELECT id,digest FROM release_content_submissions WHERE identity=?1",
                [&identity],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((id, previous)) = existing {
            ensure!(
                previous == body_digest,
                "release content identity already submitted with different content"
            );
            return Ok(id);
        }
        let count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM release_content_submissions",
            [],
            |row| row.get(0),
        )?;
        ensure!(
            count < i64::try_from(lenso_plugin_catalog::MAX_RELEASES)?,
            "release content submission limit reached"
        );
        transaction.execute(
            "INSERT INTO release_content_submissions(identity,publisher,body,digest,state) VALUES(?1,?2,?3,?4,'awaiting_review')",
            params![identity, publisher, body, body_digest]
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'submit_release_content',?2,?3)",
            params![actor, identity, i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn inspect_release_content_submission(
        &self,
        actor: &str,
        submission: i64,
    ) -> Result<(ReleaseContent, String, String)> {
        let (body, proposal_digest, state, publisher): (String, String, String, String) =
            self.connection.query_row(
                "SELECT body,digest,state,publisher FROM release_content_submissions WHERE id=?1",
                [submission],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        let owns_namespace: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM namespaces WHERE publisher=?1 AND actor=?2)",
            params![publisher, actor],
            |row| row.get(0),
        )?;
        ensure!(
            self.reviewers.contains(actor) || owns_namespace,
            "release content access denied"
        );
        ensure!(
            digest(body.as_bytes()) == proposal_digest,
            "stored release content digest mismatch"
        );
        Ok((serde_json::from_str(&body)?, proposal_digest, state))
    }

    pub fn approve_release_content(
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
            "UPDATE release_content_submissions SET state='approved',reviewer=?1,policy=?2 WHERE id=?3 AND digest=?4 AND state='awaiting_review'",
            params![actor, policy, submission, expected_digest]
        )?;
        ensure!(
            changed == 1,
            "release content changed or is not awaiting review"
        );
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'approve_release_content',?2,?3)",
            params![actor, submission.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn publish_release_content(
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
            "SELECT COALESCE(MAX(revision),0) FROM release_content_snapshots",
            [],
            |row| row.get(0),
        )?;
        ensure!(
            current == i64::try_from(expected_revision)?,
            "release content revision changed; read publication result"
        );
        let bodies: Vec<(String, String)> = transaction.prepare(
            "SELECT body,digest FROM release_content_submissions WHERE state IN ('approved','published') ORDER BY identity"
        )?.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut releases = Vec::with_capacity(bodies.len());
        for (body, reviewed_digest) in bodies {
            ensure!(
                digest(body.as_bytes()) == reviewed_digest,
                "reviewed release content digest mismatch"
            );
            let release: ReleaseContent = serde_json::from_str(&body)?;
            release.validate()?;
            base_publisher(&transaction, &release)?;
            releases.push(release);
        }
        let revision = current
            .checked_add(1)
            .context("release content revision overflow")?;
        let envelope = sign(
            &Snapshot {
                schema: SCHEMA.into(),
                catalog_id: self.catalog_id.clone(),
                revision: u64::try_from(revision)?,
                issued_at: now,
                expires_at,
                releases,
            },
            key_id,
            key,
        )?;
        transaction.execute(
            "INSERT INTO release_content_snapshots VALUES(?1,?2)",
            params![revision, envelope],
        )?;
        transaction.execute(
            "UPDATE release_content_submissions SET state='published' WHERE state='approved'",
            [],
        )?;
        transaction.execute(
            "INSERT INTO audit(actor,action,subject,at) VALUES(?1,'publish_release_content',?2,?3)",
            params![actor, revision.to_string(), i64::try_from(now)?],
        )?;
        transaction.commit()?;
        Ok(envelope)
    }

    pub fn latest_release_content(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .connection
            .query_row(
                "SELECT envelope FROM release_content_snapshots ORDER BY revision DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lenso_plugin_catalog::{Availability, linked_cargo::LinkedCargoIntegration};

    fn archive_with(entries: impl IntoIterator<Item = (String, Vec<u8>)>) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (path, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, path, &bytes[..]).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn archive() -> Vec<u8> {
        archive_with([(
            "src/index.ts".into(),
            b"export const answer = 42;\n".to_vec(),
        )])
    }

    #[test]
    fn publisher_rejects_content_the_source_app_cannot_adopt() {
        let reference = |bytes: &[u8]| Content {
            id: "starter".into(),
            kind: ContentKind::EditableTemplate,
            url: "https://example.com/starter.tar.gz".into(),
            digest: digest(bytes),
            size: bytes.len() as u64,
        };
        let reserved = archive_with([(".lenso-release-content.json".into(), b"override".to_vec())]);
        assert!(
            reference(&reserved)
                .verify_archive(&reserved, "example.editor", "1.0.0")
                .is_err()
        );
        let oversized = archive_with([("src/large.bin".into(), vec![0; 4 * 1024 * 1024 + 1])]);
        assert!(
            reference(&oversized)
                .verify_archive(&oversized, "example.editor", "1.0.0")
                .is_err()
        );
        let many = archive_with((0..513).map(|number| (format!("src/{number}.txt"), vec![b'x'])));
        assert!(
            reference(&many)
                .verify_archive(&many, "example.editor", "1.0.0")
                .is_err()
        );
        let extension = {
            let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            let mut builder = tar::Builder::new(encoder);
            let payload = vec![b'x'; 40 * 1024 * 1024];
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::GNULongName);
            header.set_size(payload.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, &payload[..]).unwrap();
            builder.into_inner().unwrap().finish().unwrap()
        };
        assert!(extension.len() < 16 * 1024 * 1024);
        assert!(
            reference(&extension)
                .verify_archive(&extension, "example.editor", "1.0.0")
                .is_err()
        );
        let mut trailing = archive();
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, &vec![0; 35 * 1024 * 1024]).unwrap();
        trailing.extend(encoder.finish().unwrap());
        assert!(trailing.len() < 16 * 1024 * 1024);
        assert!(
            reference(&trailing)
                .verify_archive(&trailing, "example.editor", "1.0.0")
                .is_err()
        );
        let mut extension = reference(&archive());
        extension.kind = ContentKind::DevelopmentExtension;
        assert!(
            extension
                .verify_archive(&archive(), "example.editor", "1.0.0")
                .is_err()
        );
        let source = archive_with([(
            "package.json".into(),
            br#"{"name":"example-editor-extension","version":"1.0.0","lenso":{"pluginId":"example.editor","runtime":"bun","conventions":[{"id":"compiler"}]}}"#.to_vec(),
        )]);
        extension.digest = digest(&source);
        extension.size = source.len() as u64;
        assert!(
            extension
                .verify_archive(&source, "example.editor", "1.0.0")
                .is_ok()
        );
    }

    #[test]
    fn legacy_read_only_database_reports_no_content_publication() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("legacy.sqlite3");
        let directory =
            Directory::open(&database, "catalog", BTreeSet::from(["reviewer".into()])).unwrap();
        directory
            .connection
            .execute_batch(
                "DROP TABLE release_content_submissions; DROP TABLE release_content_snapshots;",
            )
            .unwrap();
        drop(directory);
        let view = PublishedDirectory::open(&database, "catalog").unwrap();
        assert!(view.latest_release_content().unwrap().is_none());
        let backup = root.path().join("backup.sqlite3");
        view.backup(&backup).unwrap();
        assert!(
            PublishedDirectory::open(&backup, "catalog")
                .unwrap()
                .latest_release_content()
                .unwrap()
                .is_none()
        );
    }

    fn base() -> LinkedCargoRelease {
        LinkedCargoRelease {
            plugin_id: "example.editor".into(),
            version: "1.0.0".into(),
            publisher_id: "publisher".into(),
            title: "Editor".into(),
            summary: "Linked source release".into(),
            source_url: "https://example.com/source".into(),
            source_revision: "a".repeat(40),
            license: "MIT".into(),
            package: "example-editor".into(),
            registry_url: "https://example.com/crates/example-editor-1.0.0.crate".into(),
            crate_digest: digest(b"crate"),
            integration: LinkedCargoIntegration::LinkedPlugin,
            targets: vec!["aarch64-apple-darwin".into()],
            availability: Availability::Listed,
            documentation: vec![],
        }
    }

    #[test]
    fn reviewed_content_is_separately_signed_and_immutable() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("marketplace.sqlite3");
        let mut directory =
            Directory::open(&database, "catalog", BTreeSet::from(["reviewer".into()])).unwrap();
        directory
            .claim_namespace("reviewer", "example", "publisher", "author", 100)
            .unwrap();
        let base = base();
        let base_body = serde_json::to_string(&base).unwrap();
        directory.connection.execute(
            "INSERT INTO linked_cargo_submissions(identity,publisher,body,digest,state) VALUES(?1,?2,?3,?4,'published')",
            params!["example.editor@1.0.0", "publisher", base_body, digest(base_body.as_bytes())],
        ).unwrap();
        let archive = archive();
        let release = ReleaseContent {
            plugin_id: base.plugin_id.clone(),
            version: base.version.clone(),
            base_kind: BaseKind::LinkedCargo,
            base_release_identity: linked_identity(&base).unwrap(),
            content: vec![Content {
                id: "react-starter".into(),
                kind: ContentKind::EditableTemplate,
                url: "https://example.com/content/react-starter.tar.gz".into(),
                digest: digest(&archive),
                size: archive.len() as u64,
            }],
        };
        assert!(
            directory
                .submit_release_content("outsider", &release, &[&archive], 101)
                .is_err()
        );
        assert!(
            directory
                .submit_release_content("author", &release, &[b"wrong"], 101)
                .is_err()
        );
        let id = directory
            .submit_release_content("author", &release, &[&archive], 101)
            .unwrap();
        assert_eq!(
            directory
                .submit_release_content("author", &release, &[&archive], 101)
                .unwrap(),
            id
        );
        let (_, proposal_digest, state) = directory
            .inspect_release_content_submission("author", id)
            .unwrap();
        assert_eq!(state, "awaiting_review");
        assert!(
            directory
                .approve_release_content("reviewer", id, "sha256:wrong", "v2", 102)
                .is_err()
        );
        directory
            .approve_release_content("reviewer", id, &proposal_digest, "v2", 102)
            .unwrap();
        let key = SigningKey::from_bytes(&[7; 32]);
        let bytes = directory
            .publish_release_content("reviewer", 0, 103, 200, "key", &key)
            .unwrap();
        let trust = Trust {
            catalog_id: "catalog".into(),
            keys: BTreeMap::from([("key".into(), key.verifying_key())]),
        };
        let verified = verify(&bytes, &trust, 150).unwrap();
        assert_eq!(verified.releases, vec![release.clone()]);
        assert_eq!(
            PublishedDirectory::open(&database, "catalog")
                .unwrap()
                .latest_release_content()
                .unwrap()
                .unwrap()
                .as_bytes(),
            bytes
        );
        assert!(
            directory
                .publish_release_content("reviewer", 0, 104, 200, "key", &key)
                .is_err()
        );
        let mut changed = release;
        changed.content[0].url = "https://example.com/content/changed.tar.gz".into();
        assert!(
            directory
                .submit_release_content("author", &changed, &[&archive], 105)
                .is_err()
        );
    }
}
