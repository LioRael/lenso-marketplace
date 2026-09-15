import assert from "node:assert/strict";
// Render a reviewable deployment configuration from explicit public inputs.
// This command never creates resources, handles private keys, or deploys.
import { readFileSync, writeFileSync } from "node:fs";

const [inputPath, outputPath] = process.argv.slice(2);
assert.ok(
  inputPath && outputPath,
  "usage: node proof/deployment-config.mjs inputs.json output.json"
);
const input = JSON.parse(readFileSync(inputPath, "utf-8"));
for (const key of [
  "worker",
  "hostname",
  "database_name",
  "database_id",
  "bucket_name",
  "catalog_id",
  "key_id",
  "public_key_hex",
]) {
  assert.ok(
    typeof input[key] === "string" && input[key].length > 0,
    `missing ${key}`
  );
}
assert.ok(
  /^[a-z0-9][a-z0-9-]{1,62}$/u.test(input.worker),
  "invalid Worker name"
);
assert.ok(
  /^[a-z0-9][a-z0-9.-]+\.[a-z]{2,}$/u.test(input.hostname),
  "invalid hostname"
);
assert.ok(
  input.hostname !== "catalog.lenso.dev",
  "legacy catalog domain is outside this rollout"
);
assert.ok(
  /^[a-f0-9]{8}-(?:[a-f0-9]{4}-){3}[a-f0-9]{12}$/u.test(input.database_id),
  "invalid D1 ID"
);
assert.ok(
  /^[a-f0-9]{64}$/u.test(input.public_key_hex),
  "expected Ed25519 public key hex"
);
assert.ok(
  input.public_key_hex !==
    "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737",
  "proof trust is not production trust"
);
assert.ok(
  ![
    input.worker,
    input.database_name,
    input.bucket_name,
    input.catalog_id,
    input.key_id,
  ].some((value) => /proof|test-key|workers-g3/iu.test(value)),
  "proof resource or identity rejected"
);
assert.ok(
  Number.isInteger(input.cpu_ms) && input.cpu_ms > 0 && input.cpu_ms <= 1000,
  "explicit qualified CPU ceiling required (1..1000ms)"
);
const config = {
  compatibility_date: "2026-07-08",
  compatibility_flags: [
    "global_fetch_strictly_public",
    "enable_request_signal",
  ],
  d1_databases: [
    {
      binding: "MARKETPLACE_DB",
      database_id: input.database_id,
      database_name: input.database_name,
      migrations_dir: "migrations",
    },
  ],
  limits: { cpu_ms: input.cpu_ms },
  main: "worker.mjs",
  name: input.worker,
  observability: { enabled: true },
  preview_urls: false,
  r2_buckets: [
    { binding: "MARKETPLACE_OBJECTS", bucket_name: input.bucket_name },
  ],
  routes: [{ custom_domain: true, pattern: input.hostname }],
  vars: {
    CATALOG_ID: input.catalog_id,
    CATALOG_KEY_ID: input.key_id,
    CATALOG_PUBLIC_KEY: input.public_key_hex,
  },
  workers_dev: false,
};
writeFileSync(outputPath, `${JSON.stringify(config, null, 2)}\n`, {
  flag: "wx",
});
console.log(
  "Configuration written for review; no deployment performed. Keep it beside worker.mjs."
);
