import assert from "node:assert/strict";
import { test } from "node:test";

import { channels, legacySchemas, schema } from "../keyless/catalog.mjs";
import { digest } from "../keyless/verify.mjs";
import { promoteKeyless } from "./promote-keyless.mjs";

function fixture() {
  return {
    schema,
    catalog_id: "lenso-official-v2",
    revision: 1,
    issued_at: 1,
    releases: [
      {
        channel: "package",
        plugin_id: "test.plugin",
        version: "1.0.0",
        record: {
          plugin_id: "test.plugin",
          version: "1.0.0",
          publisher_id: "test",
          title: "Test",
          summary: "Fixture",
          license: "MIT",
          source_revision: "a".repeat(40),
          source_url: "https://github.com/example/test",
          distributions: [
            {
              kind: "npm_package",
              package: "@test/plugin",
              version: "1.0.0",
              registry_url: "https://registry.npmjs.org",
              integrity: `sha256:${"b".repeat(64)}`,
            },
          ],
        },
      },
    ],
    statuses: [{ plugin_id: "test.plugin", version: "1.0.0", state: "listed" }],
    legacy_sources: Object.fromEntries(
      channels.map((channel) => [
        channel,
        {
          schema: legacySchemas[channel],
          revision: 1,
          payload_digest: `sha256:${"c".repeat(64)}`,
        },
      ])
    ),
  };
}

function setup() {
  let head = null;
  const objects = new Map();
  const calls = [];
  const database = {
    prepare(sql) {
      return {
        bind(...params) {
          return {
            async first() {
              calls.push("read");
              return head;
            },
            async run() {
              calls.push("cas");
              if (sql.startsWith("INSERT")) {
                if (head) return { meta: { changes: 0 } };
                const [
                  catalog_id,
                  revision,
                  catalog_digest,
                  catalog_size,
                  bundle_digest,
                  bundle_size,
                  source_sha,
                ] = params;
                head = {
                  catalog_id,
                  revision,
                  catalog_digest,
                  catalog_size,
                  bundle_digest,
                  bundle_size,
                  source_sha,
                };
              } else {
                const [
                  revision,
                  catalog_digest,
                  catalog_size,
                  bundle_digest,
                  bundle_size,
                  source_sha,
                  catalog_id,
                  oldRevision,
                  oldDigest,
                ] = params;
                if (
                  head?.revision !== oldRevision ||
                  head?.catalog_digest !== oldDigest
                )
                  return { meta: { changes: 0 } };
                head = {
                  catalog_id,
                  revision,
                  catalog_digest,
                  catalog_size,
                  bundle_digest,
                  bundle_size,
                  source_sha,
                };
              }
              return { meta: { changes: 1 } };
            },
          };
        },
      };
    },
  };
  const bucket = {
    async get(key) {
      const bytes = objects.get(key);
      return bytes
        ? { size: bytes.length, body: new Response(bytes).body }
        : null;
    },
    async put(key, bytes, options) {
      calls.push("put");
      assert.equal(options.onlyIf.etagDoesNotMatch, "*");
      if (objects.has(key)) return null;
      objects.set(key, bytes);
      return { key };
    },
  };
  const config = {
    catalogBytes: Buffer.from(JSON.stringify(fixture())),
    bundleBytes: Buffer.from("test bundle; not cryptographic proof"),
    sourceSha: "d".repeat(40),
    expected: null,
    bucket,
    database,
    verify: async () => {
      calls.push("verify");
    },
  };
  return { config, calls, objects };
}

test("verified bytes are uploaded create-only before one atomic head promotion", async () => {
  const { config, calls } = setup();
  const receipt = await promoteKeyless(config);
  assert.equal(receipt.catalog_digest, digest(config.catalogBytes));
  assert.equal(receipt.bundle_digest, digest(config.bundleBytes));
  assert.deepEqual(calls, ["verify", "read", "put", "put", "cas", "read"]);
});

test("failed verification performs no remote read or write", async () => {
  const { config, calls } = setup();
  config.verify = async () => {
    throw new Error("Invalid provenance");
  };
  await assert.rejects(promoteKeyless(config));
  assert.deepEqual(calls, []);
});

test("changed expected head and reused revisions fail before uploads", async () => {
  const { config, calls } = setup();
  const receipt = await promoteKeyless(config);
  calls.length = 0;
  config.expected = { revision: 2, catalog_digest: receipt.catalog_digest };
  await assert.rejects(promoteKeyless(config));
  assert.deepEqual(calls, ["verify", "read"]);
  config.expected.revision = 1;
  await assert.rejects(promoteKeyless(config), /Revision must increase/);
});

test("existing releases cannot be overwritten or dropped; status can change", async () => {
  const { config } = setup();
  const receipt = await promoteKeyless(config);
  config.expected = {
    revision: receipt.revision,
    catalog_digest: receipt.catalog_digest,
  };
  const next = fixture();
  next.revision = 2;
  next.releases[0].record.title = "Replacement";
  config.catalogBytes = Buffer.from(JSON.stringify(next));
  await assert.rejects(promoteKeyless(config), /immutable/);
  next.releases[0].record.title = "Test";
  next.statuses[0] = {
    ...next.statuses[0],
    state: "revoked",
    reason: "Security review",
  };
  config.catalogBytes = Buffer.from(JSON.stringify(next));
  const promoted = await promoteKeyless(config);
  assert.equal(promoted.revision, 2);
});

test("unconfirmed CAS is not blindly retried", async () => {
  const { config, calls } = setup();
  config.database.prepare = (sql) => ({
    bind: () => ({
      first: async () => null,
      run: async () => {
        calls.push("cas");
        return { meta: { changes: 0 } };
      },
    }),
  });
  await assert.rejects(promoteKeyless(config), /unconfirmed/);
  assert.equal(calls.filter((value) => value === "cas").length, 1);
});

test("revoked exact versions cannot be relisted", async () => {
  const { config } = setup();
  const first = fixture();
  first.statuses[0] = {
    ...first.statuses[0],
    state: "revoked",
    reason: "Security review",
  };
  config.catalogBytes = Buffer.from(JSON.stringify(first));
  const receipt = await promoteKeyless(config);
  config.expected = {
    revision: receipt.revision,
    catalog_digest: receipt.catalog_digest,
  };
  const next = fixture();
  next.revision = 2;
  config.catalogBytes = Buffer.from(JSON.stringify(next));
  await assert.rejects(promoteKeyless(config), /Revocation is terminal/);
});
