import assert from "node:assert/strict";
import { test } from "node:test";

import { channels, legacySchemas, schema } from "../keyless/catalog.mjs";
import { digest } from "../keyless/verify.mjs";
import { promoteKeyless } from "./promote-keyless.mjs";

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
        summary: "Fixture",
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

const setup = () => {
  let head = null;
  const objects = new Map();
  const calls = [];
  const database = {
    prepare(sql) {
      return {
        bind(...params) {
          return {
            first() {
              calls.push("read");
              return Promise.resolve(head);
            },
            run() {
              calls.push("cas");
              const insert = sql.startsWith("INSERT");
              if (insert && head) {
                return Promise.resolve({ meta: { changes: 0 } });
              }
              if (
                !insert &&
                (head?.revision !== params[7] ||
                  head?.catalog_digest !== params[8])
              ) {
                return Promise.resolve({ meta: { changes: 0 } });
              }
              const [
                catalog_id,
                revision,
                catalog_digest,
                catalog_size,
                bundle_digest,
                bundle_size,
                source_sha,
              ] = insert
                ? params.slice(0, 7)
                : [params[6], ...params.slice(0, 6)];
              head = {
                bundle_digest,
                bundle_size,
                catalog_digest,
                catalog_id,
                catalog_size,
                revision,
                source_sha,
              };
              return Promise.resolve({ meta: { changes: 1 } });
            },
          };
        },
      };
    },
  };
  const bucket = {
    get(key) {
      const bytes = objects.get(key);
      return Promise.resolve(
        bytes ? { body: new Response(bytes).body, size: bytes.length } : null
      );
    },
    put(key, bytes, options) {
      calls.push("put");
      assert.equal(options.onlyIf.etagDoesNotMatch, "*");
      if (objects.has(key)) {
        return Promise.resolve(null);
      }
      objects.set(key, bytes);
      return Promise.resolve({ key });
    },
  };
  const config = {
    bucket,
    bundleBytes: Buffer.from("test bundle; not cryptographic proof"),
    catalogBytes: Buffer.from(JSON.stringify(fixture())),
    database,
    expected: null,
    sourceSha: "d".repeat(40),
    verify: () => {
      calls.push("verify");
      return Promise.resolve();
    },
  };
  return { calls, config, objects };
};

test("verified bytes are uploaded create-only before one atomic head promotion", async () => {
  const { config, calls } = setup();
  const receipt = await promoteKeyless(config);
  assert.equal(receipt.catalog_digest, digest(config.catalogBytes));
  assert.equal(receipt.bundle_digest, digest(config.bundleBytes));
  assert.deepEqual(calls, ["verify", "read", "put", "put", "cas", "read"]);
});

test("failed verification performs no remote read or write", async () => {
  const { config, calls } = setup();
  config.verify = () => Promise.reject(new Error("Invalid provenance"));
  await assert.rejects(promoteKeyless(config));
  assert.deepEqual(calls, []);
});

test("changed expected head and reused revisions fail before uploads", async () => {
  const { config, calls } = setup();
  const receipt = await promoteKeyless(config);
  calls.length = 0;
  config.expected = { catalog_digest: receipt.catalog_digest, revision: 2 };
  await assert.rejects(promoteKeyless(config));
  assert.deepEqual(calls, ["verify", "read"]);
  config.expected.revision = 1;
  await assert.rejects(promoteKeyless(config), /Revision must increase/u);
});

test("existing releases cannot be overwritten or dropped; status can change", async () => {
  const { config } = setup();
  const receipt = await promoteKeyless(config);
  config.expected = {
    catalog_digest: receipt.catalog_digest,
    revision: receipt.revision,
  };
  const next = fixture();
  next.revision = 2;
  next.releases[0].record.title = "Replacement";
  config.catalogBytes = Buffer.from(JSON.stringify(next));
  await assert.rejects(promoteKeyless(config), /immutable/u);
  next.releases[0].record.title = "Test";
  next.statuses[0] = {
    ...next.statuses[0],
    reason: "Security review",
    state: "revoked",
  };
  config.catalogBytes = Buffer.from(JSON.stringify(next));
  const promoted = await promoteKeyless(config);
  assert.equal(promoted.revision, 2);
});

test("unconfirmed CAS is not blindly retried", async () => {
  const { config, calls } = setup();
  config.database.prepare = () => ({
    bind: () => ({
      first: () => Promise.resolve(null),
      run: () => {
        calls.push("cas");
        return Promise.resolve({ meta: { changes: 0 } });
      },
    }),
  });
  await assert.rejects(promoteKeyless(config), /unconfirmed/u);
  assert.equal(calls.filter((value) => value === "cas").length, 1);
});

test("revoked exact versions cannot be relisted", async () => {
  const { config } = setup();
  const first = fixture();
  first.statuses[0] = {
    ...first.statuses[0],
    reason: "Security review",
    state: "revoked",
  };
  config.catalogBytes = Buffer.from(JSON.stringify(first));
  const receipt = await promoteKeyless(config);
  config.expected = {
    catalog_digest: receipt.catalog_digest,
    revision: receipt.revision,
  };
  const next = fixture();
  next.revision = 2;
  config.catalogBytes = Buffer.from(JSON.stringify(next));
  await assert.rejects(promoteKeyless(config), /Revocation is terminal/u);
});
