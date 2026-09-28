import assert from "node:assert/strict";
import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

const fixture = (name) =>
  readFileSync(new URL(`../../tests/fixtures/d16/${name}`, import.meta.url));
const digest = (bytes) =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const publicKey = createPublicKey({
  key: Buffer.concat([
    Buffer.from("302a300506032b6570032100", "hex"),
    Buffer.from(
      "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c",
      "hex"
    ),
  ]),
  format: "der",
  type: "spki",
});
const signed = (name, schema) => {
  const envelope = JSON.parse(fixture(name));
  assert.equal(envelope.key_id, "key");
  const payload = Buffer.from(envelope.payload_base64, "base64");
  const message = Buffer.concat([
    Buffer.from(`${schema}\0${envelope.key_id}\0`),
    payload,
  ]);
  assert.ok(
    verify(
      null,
      message,
      publicKey,
      Buffer.from(envelope.signature_base64, "base64")
    ),
    `${name} has a valid detached signature`
  );
  const snapshot = JSON.parse(payload);
  assert.equal(snapshot.schema, schema);
  assert.equal(snapshot.catalog_id, "catalog");
  assert.equal(snapshot.revision, 1);
  assert.equal(snapshot.issued_at, 103);
  assert.equal(snapshot.expires_at, 200);
  return snapshot;
};

test("D16 signed package and content fixtures join by exact immutable identity", () => {
  const base = signed(
    "package-snapshot.envelope.json",
    "lenso.marketplace.package-snapshot.v1"
  ).releases[0];
  assert.match(base.source_revision, /^[a-f0-9]{40}$/u);
  assert.deepEqual(base, JSON.parse(fixture("package-release.json")));
  const baseIdentity = digest(
    Buffer.from(
      JSON.stringify([
        base.plugin_id,
        base.version,
        base.publisher_id,
        base.title,
        base.summary,
        base.source_url,
        base.source_revision,
        base.license,
        base.distributions,
      ])
    )
  );
  const attached = signed(
    "package-content.envelope.json",
    "lenso.marketplace.release-content.v2"
  ).releases[0];
  assert.equal(attached.base_kind, "package");
  assert.equal(attached.base_release_identity, baseIdentity);
  assert.equal(attached.plugin_id, base.plugin_id);
  assert.equal(attached.version, base.version);
  assert.equal(attached.metadata, undefined);

  const standalone = signed(
    "content-only.envelope.json",
    "lenso.marketplace.release-content.v2"
  ).releases[0];
  assert.equal(standalone.base_kind, "content_only");
  assert.notEqual(standalone.plugin_id, base.plugin_id);
  const metadata = standalone.metadata;
  assert.equal(metadata.publisher_id, "publisher");
  assert.ok(
    metadata.documentation.some((doc) => doc.topic === "getting-started")
  );
  const documents = metadata.documentation.map((doc) => [
    doc.id,
    doc.revision,
    doc.language,
    doc.topic,
    doc.target ?? null,
    doc.url,
    doc.digest,
    doc.size,
    doc.media_type,
  ]);
  const content = standalone.content.map((item) => [
    item.id,
    item.kind,
    item.url,
    item.digest,
    item.size,
  ]);
  const metadataTuple = [
    metadata.publisher_id,
    metadata.title,
    metadata.summary,
    metadata.source_url,
    metadata.source_revision,
    metadata.license,
    documents,
  ];
  assert.equal(
    standalone.base_release_identity,
    digest(
      Buffer.from(
        JSON.stringify([
          standalone.plugin_id,
          standalone.version,
          metadataTuple,
          content,
        ])
      )
    )
  );
  const markdown = fixture("getting-started.md");
  for (const doc of [...base.documentation, ...metadata.documentation]) {
    assert.equal(doc.size, markdown.length);
    assert.equal(doc.digest, digest(markdown));
    assert.equal(doc.media_type, "text/markdown");
  }
});
