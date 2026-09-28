import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
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
