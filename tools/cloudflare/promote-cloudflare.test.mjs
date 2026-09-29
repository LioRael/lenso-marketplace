import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(
  new URL("promote-cloudflare.mjs", import.meta.url)
);

test("release-content selects its own publisher export and verifier", (t) => {
  const directory = mkdtempSync(
    join(tmpdir(), "lenso-release-content-promote-")
  );
  t.after(() => rmSync(directory, { recursive: true }));
  const publisherBinary = join(directory, "publisher.mjs");
  const publisherConfig = join(directory, "publisher.json");
  const calls = join(directory, "calls.txt");
  const config = join(directory, "promotion.json");
  writeFileSync(
    publisherBinary,
    `#!/usr/bin/env node
import { appendFileSync } from "node:fs";
const operation = process.argv[3];
appendFileSync(process.env.PUBLISHER_CALLS, operation + "\\n");
process.stdout.write(JSON.stringify(operation === "export-release-content"
  ? { envelope: '{"catalog_id":"catalog","revision":1,"expires_at":1000}' }
  : { catalog_id: "wrong-catalog", revision: 1, expires_at: 1000 }));
`
  );
  chmodSync(publisherBinary, 0o700);
  writeFileSync(publisherConfig, "{}");
  writeFileSync(
    config,
    JSON.stringify({
      accountId: "a".repeat(32),
      bucketName: "test-bucket",
      catalogId: "catalog",
      databaseId: "11111111-1111-4111-8111-111111111111",
      expected: null,
      kind: "release-content",
      publisherBinary,
      publisherConfig,
    })
  );
  const result = spawnSync(process.execPath, [script, config], {
    encoding: "utf-8",
    env: {
      ...process.env,
      MARKETPLACE_D1_TOKEN: "test-token",
      MARKETPLACE_R2_ACCESS_KEY_ID: "test-access-key",
      MARKETPLACE_R2_SECRET_ACCESS_KEY: "test-secret-key",
      PUBLISHER_CALLS: calls,
    },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /invalid verified publication identity/u);
  assert.deepEqual(readFileSync(calls, "utf-8").trim().split("\n"), [
    "export-release-content",
    "verify-release-content",
  ]);
});

test("parallel promotion rejects the legacy D1 before publisher execution", (t) => {
  const directory = mkdtempSync(join(tmpdir(), "lenso-parallel-promote-"));
  t.after(() => rmSync(directory, { recursive: true }));
  const publisherBinary = join(directory, "publisher.mjs");
  const publisherConfig = join(directory, "publisher.json");
  const deploymentConfig = join(directory, "wrangler.json");
  const config = join(directory, "promotion.json");
  const calls = join(directory, "calls.txt");
  writeFileSync(
    publisherBinary,
    `#!/usr/bin/env node
import { writeFileSync } from "node:fs";
writeFileSync(process.env.PUBLISHER_CALLS, "called");
`
  );
  chmodSync(publisherBinary, 0o700);
  writeFileSync(
    publisherConfig,
    JSON.stringify({
      catalog_id: "lenso-official-v2",
      database: join(directory, "publisher.sqlite3"),
      key_id: "lenso-marketplace-v2-2026",
      public_key_hex: "a".repeat(64),
    })
  );
  writeFileSync(
    deploymentConfig,
    JSON.stringify({
      account_id: "a".repeat(32),
      d1_databases: [
        {
          database_id: "cb599e1c-fb55-44f3-850b-679f2f934b67",
          database_name: "lenso-marketplace-v2-production",
        },
      ],
      name: "lenso-marketplace-v2",
      r2_buckets: [{ bucket_name: "lenso-marketplace-v2-production" }],
      routes: [{ custom_domain: true, pattern: "marketplace-v2.lenso.dev" }],
      vars: {
        CATALOG_ID: "lenso-official-v2",
        CATALOG_KEY_ID: "lenso-marketplace-v2-2026",
        CATALOG_PUBLIC_KEY: "a".repeat(64),
      },
    })
  );
  writeFileSync(
    config,
    JSON.stringify({
      accountId: "a".repeat(32),
      bucketName: "lenso-marketplace-v2-production",
      catalogId: "lenso-official-v2",
      databaseId: "cb599e1c-fb55-44f3-850b-679f2f934b67",
      deploymentConfig,
      deploymentTrack: "parallel-new-root",
      expected: null,
      legacyPublicKeyHex: "b".repeat(64),
      publisherBinary,
      publisherConfig,
    })
  );
  const result = spawnSync(process.execPath, [script, config], {
    encoding: "utf-8",
    env: { ...process.env, PUBLISHER_CALLS: calls },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /parallel root reuses legacy database_id/u);
  assert.equal(existsSync(calls), false);
});
