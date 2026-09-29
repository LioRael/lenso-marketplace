import assert from "node:assert/strict";
import { test } from "node:test";

import {
  channels,
  legacySchemas,
  loadReviewedCatalog,
  schema,
  validateCatalog,
} from "./catalog.mjs";
import { digest } from "./verify.mjs";

const fixture = () => ({
  catalog_id: "lenso-official-v2",
  issued_at: 1,
  legacy_sources: Object.fromEntries(
    channels.map((channel) => [
      channel,
      {
        payload_digest: `sha256:${"c".repeat(64)}`,
        revision: 1,
        schema: legacySchemas[channel],
      },
    ])
  ),
  releases: [
    {
      channel: "package",
      plugin_id: "test.plugin",
      record: {
        distributions: [
          {
            integrity: `sha256:${"b".repeat(64)}`,
            kind: "npm_package",
            package: "@test/plugin",
            registry_url: "https://registry.npmjs.org",
            version: "1.0.0",
          },
        ],
        license: "MIT",
        plugin_id: "test.plugin",
        publisher_id: "test",
        source_revision: "a".repeat(40),
        source_url: "https://github.com/example/test",
        summary: "Test fixture",
        title: "Test",
        version: "1.0.0",
      },
      version: "1.0.0",
    },
  ],
  revision: 1,
  schema,
  statuses: [{ plugin_id: "test.plugin", state: "listed", version: "1.0.0" }],
});

test("catalog preserves typed records without expiry or a self-referential source commit", () => {
  const catalog = fixture();
  assert.equal(validateCatalog(catalog), catalog);
  for (const field of ["expires_at", "source_commit", "signature_base64"]) {
    assert.throws(() => validateCatalog({ ...fixture(), [field]: "invalid" }));
  }
});

test("catalog rejects missing, duplicate and unmatched status identities", () => {
  const catalog = fixture();
  catalog.statuses = [];
  assert.throws(() => validateCatalog(catalog));
  catalog.statuses = [
    { plugin_id: "test.other", state: "listed", version: "1.0.0" },
  ];
  assert.throws(() => validateCatalog(catalog));
  const duplicate = fixture();
  duplicate.releases.push(structuredClone(duplicate.releases[0]));
  assert.throws(() => validateCatalog(duplicate));
});

test("yanking and revocation require explicit reasons", () => {
  for (const state of ["yanked", "revoked"]) {
    const catalog = fixture();
    catalog.statuses[0].state = state;
    assert.throws(() => validateCatalog(catalog));
    catalog.statuses[0].reason = "Operator security decision";
    validateCatalog(catalog);
  }
});

test("record identity, exact digest, secure URL and bounded metadata are mandatory", () => {
  const mutations = [
    (record) => {
      record.version = "2.0.0";
    },
    (record) => {
      record.distributions[0].integrity = "latest";
    },
    (record) => {
      record.distributions[0].registry_url = "http://registry.npmjs.org";
    },
    (record) => {
      record.source_revision = "main";
    },
    (record) => {
      record.title = "x".repeat(16_385);
    },
  ];
  for (const mutate of mutations) {
    const catalog = fixture();
    mutate(catalog.releases[0].record);
    assert.throws(() => validateCatalog(catalog));
  }
});

test("reviewed original bytes cannot be silently reserialized", () => {
  const bytes = Buffer.from(JSON.stringify(fixture()));
  loadReviewedCatalog(bytes, digest(bytes));
  assert.throws(() =>
    loadReviewedCatalog(
      Buffer.concat([bytes, Buffer.from("\n")]),
      digest(bytes)
    )
  );
  assert.throws(() =>
    loadReviewedCatalog(Buffer.alloc(4 * 1024 * 1024 + 1), digest(bytes))
  );
});
