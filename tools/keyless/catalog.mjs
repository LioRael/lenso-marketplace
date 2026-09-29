import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { digest } from "./verify.mjs";

export const schema = "lenso.marketplace.keyless-catalog.v1";
export const channels = ["portable", "linked_cargo", "package", "release_content"];
export const legacySchemas = Object.freeze({
  portable: "lenso.marketplace.snapshot.v1",
  linked_cargo: "lenso.marketplace.linked-cargo-snapshot.v1",
  package: "lenso.marketplace.package-snapshot.v1",
  release_content: "lenso.marketplace.release-content.v2",
});
const sha256 = /^sha256:[a-f0-9]{64}$/u;

function fields(value, allowed, required = allowed) {
  assert.ok(value && typeof value === "object" && !Array.isArray(value), "Expected an object");
  for (const key of Object.keys(value)) assert.ok(allowed.includes(key), `Unknown field ${key}`);
  for (const key of required) assert.ok(Object.hasOwn(value, key), `Missing field ${key}`);
}

function text(value, limit = 2048) {
  assert.ok(typeof value === "string" && value.length > 0 && value.length <= limit, "Unbounded text");
  assert.ok(!/[\u0000-\u001f]/u.test(value), "Control characters are not allowed");
}

function https(value) {
  text(value);
  const url = new URL(value);
  assert.ok(url.protocol === "https:" && url.hostname && !url.username && !url.password && !url.hash,
    "Expected a credential-free HTTPS URL");
}

function boundedRecord(value, depth = 0) {
  assert.ok(depth <= 16, "Record nesting exceeds limit");
  if (typeof value === "string") { text(value, 16_384); return; }
  if (typeof value === "boolean" || value === null) return;
  if (typeof value === "number") { assert.ok(Number.isSafeInteger(value) && value >= 0); return; }
  if (Array.isArray(value)) {
    assert.ok(value.length <= 1024);
    for (const child of value) boundedRecord(child, depth + 1);
    return;
  }
  assert.ok(value && typeof value === "object");
  assert.ok(Object.keys(value).length <= 128);
  for (const [key, child] of Object.entries(value)) {
    text(key, 128);
    assert.ok(!["__proto__", "constructor", "prototype"].includes(key));
    if (["digest", "crate_digest", "integrity", "artifact_digest", "manifest_digest"].includes(key)) {
      assert.match(child, sha256, `Invalid ${key}`);
    }
    if (key === "source_revision") assert.match(child, /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/u);
    if (key === "url" || key.endsWith("_url")) https(child);
    if (key === "size") assert.ok(Number.isSafeInteger(child) && child > 0 && child <= 512 * 1024 * 1024);
    boundedRecord(child, depth + 1);
  }
}

function validateRecord(release) {
  const record = release.record;
  boundedRecord(record);
  assert.equal(record.plugin_id, release.plugin_id);
  assert.equal(record.version, release.version);
  const metadata = release.channel === "release_content" ? record.metadata : record;
  assert.ok(metadata && typeof metadata === "object");
  for (const key of ["publisher_id", "title", "summary", "license"]) text(metadata[key], 1024);
  https(metadata.source_url);
  assert.match(metadata.source_revision, /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/u);
  if (metadata.documentation !== undefined) {
    assert.ok(Array.isArray(metadata.documentation) && metadata.documentation.length <= 64);
    const documents = new Set();
    for (const document of metadata.documentation) {
      for (const key of ["id", "revision", "language", "topic"]) text(document[key], 128);
      const identity = `${document.id}@${document.revision}`;
      assert.ok(!documents.has(identity), "Duplicate documentation identity");
      documents.add(identity);
      https(document.url);
      assert.match(document.digest, sha256);
      assert.equal(document.media_type, "text/markdown");
      assert.ok(Number.isSafeInteger(document.size) && document.size > 0 && document.size <= 4 * 1024 * 1024);
    }
  }
  if (release.channel === "linked_cargo") {
    assert.match(record.crate_digest, sha256);
    text(record.package, 128);
    assert.ok(["host_provided", "linked_plugin"].includes(record.integration));
    assert.ok(Array.isArray(record.targets) && record.targets.length > 0);
  } else if (release.channel === "package") {
    assert.ok(Array.isArray(record.distributions) && record.distributions.length > 0);
    for (const distribution of record.distributions) {
      assert.equal(distribution.kind, "npm_package");
      assert.equal(distribution.version, record.version);
      assert.match(distribution.integrity, sha256);
      text(distribution.package, 128);
      https(distribution.registry_url);
    }
  } else if (release.channel === "release_content") {
    assert.equal(record.base_kind, "content_only", "Attached source content requires a separately versioned protocol");
    assert.ok(record.metadata && Array.isArray(record.content) && record.content.length > 0);
    for (const content of record.content) {
      assert.ok(["editable_template", "development_extension"].includes(content.kind));
      assert.match(content.digest, sha256);
      https(content.url);
      assert.ok(content.size > 0 && content.size <= 16 * 1024 * 1024);
    }
  } else {
    assert.ok(record.artifact, "Portable release requires exact artifact identity");
    assert.match(record.artifact.digest, sha256);
    assert.match(record.artifact.manifest_digest, sha256);
    https(record.artifact.url);
    assert.ok(record.artifact.size > 0);
  }
}

export function validateCatalog(input) {
  fields(input, ["schema", "catalog_id", "revision", "issued_at", "releases", "statuses", "legacy_sources"]);
  assert.equal(input.schema, schema);
  assert.equal(input.catalog_id, "lenso-official-v2");
  assert.ok(Number.isSafeInteger(input.revision) && input.revision > 0);
  assert.ok(Number.isSafeInteger(input.issued_at) && input.issued_at > 0);
  assert.ok(Array.isArray(input.releases) && input.releases.length > 0 && input.releases.length <= 10_000);
  const identities = new Set();
  for (const release of input.releases) {
    fields(release, ["channel", "plugin_id", "version", "record"]);
    assert.ok(channels.includes(release.channel));
    assert.match(release.plugin_id, /^[a-z][a-z0-9_-]*(?:\.[a-z][a-z0-9_-]*)+$/u);
    assert.match(release.version, /^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?(?:\+[a-zA-Z0-9.-]+)?$/u);
    const identity = `${release.plugin_id}@${release.version}`;
    assert.ok(!identities.has(identity), "Duplicate global release identity");
    identities.add(identity);
    validateRecord(release);
  }
  assert.ok(Array.isArray(input.statuses) && input.statuses.length === identities.size);
  const statuses = new Set();
  for (const status of input.statuses) {
    fields(status, ["plugin_id", "version", "state", "reason"], ["plugin_id", "version", "state"]);
    const identity = `${status.plugin_id}@${status.version}`;
    assert.ok(identities.has(identity) && !statuses.has(identity), "Status identity mismatch");
    statuses.add(identity);
    assert.ok(["listed", "yanked", "revoked"].includes(status.state));
    if (status.reason !== undefined) text(status.reason, 1024);
    if (status.state !== "listed") text(status.reason, 1024);
  }
  fields(input.legacy_sources, channels);
  for (const [channel, source] of Object.entries(input.legacy_sources)) {
    fields(source, ["schema", "revision", "payload_digest"]);
    assert.equal(source.schema, legacySchemas[channel]);
    assert.ok(Number.isSafeInteger(source.revision) && source.revision > 0);
    assert.match(source.payload_digest, sha256);
  }
  return input;
}

export function loadReviewedCatalog(bytes, expectedDigest) {
  assert.ok(bytes.length <= 4 * 1024 * 1024, "Catalog exceeds four MiB");
  assert.match(expectedDigest, /^[a-f0-9]{64}$/u);
  assert.equal(digest(bytes), expectedDigest, "Reviewed catalog digest mismatch");
  return validateCatalog(JSON.parse(bytes.toString("utf8")));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [path, expectedDigest, ...extra] = process.argv.slice(2);
  assert.equal(extra.length, 0);
  loadReviewedCatalog(await readFile(path), expectedDigest);
  console.log("Reviewed catalog validated");
}
