import assert from "node:assert/strict";
// Render a reviewable deployment configuration from explicit public inputs.
// This command never creates resources, handles private keys, or deploys.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const [inputPath, outputPath] = process.argv.slice(2);
assert.ok(
  inputPath && outputPath,
  "usage: node tools/cloudflare/deployment-config.mjs inputs.json output.json"
);
const input = JSON.parse(readFileSync(inputPath, "utf-8"));
if ("workers_dev" in input) {
  assert.equal(
    typeof input.workers_dev,
    "boolean",
    "workers_dev must be a boolean when provided"
  );
}
for (const key of [
  "account_id",
  "environment",
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
assert.equal(input.environment, "production", "environment must be production");
assert.match(
  input.account_id,
  /^[a-f0-9]{32}$/u,
  "invalid Cloudflare account ID"
);
assert.ok(
  /^[a-z0-9][a-z0-9-]{1,62}$/u.test(input.worker),
  "invalid Worker name"
);
assert.ok(
  /^[a-z0-9][a-z0-9.-]+\.[a-z]{2,}$/u.test(input.hostname),
  "invalid hostname"
);
assert.ok(
  !input.hostname.endsWith(".workers.dev") &&
    !/^(?:localhost|127\.0\.0\.1)$/u.test(input.hostname),
  "production hostname must be a custom domain"
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
  ![
    "2b913921-3f0a-43b4-ae8f-cb219c21db81",
    "f9d225cb-88e8-4681-858a-0376b26ab7e0",
    "256d0844-cff5-4ead-8770-ab62c7f181d1",
  ].includes(input.database_id),
  "known proof D1 resource is not production"
);
assert.match(
  input.bucket_name,
  /^[a-z0-9][a-z0-9-]{1,62}$/u,
  "invalid R2 bucket name"
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
  input.public_key_hex !==
    "8d47ca04c5564c517371edc4540ee0945e4b15c3843b6a04a753d11b257b27c9",
  "publisher proof trust is not production trust"
);
assert.ok(
  ![
    input.account_id,
    input.worker,
    input.database_name,
    input.bucket_name,
    input.catalog_id,
    input.key_id,
  ].some((value) => /proof|test-key|workers-g3|recovery/iu.test(value)),
  "proof resource or identity rejected"
);
assert.ok(
  Number.isInteger(input.cpu_ms) && input.cpu_ms > 0 && input.cpu_ms <= 1000,
  "explicit qualified CPU ceiling required (1..1000ms)"
);
const config = {
  account_id: input.account_id,
  assets: {
    binding: "ASSETS",
    directory: fileURLToPath(
      new URL("../../plugins/web/ui/dist", import.meta.url)
    ),
    not_found_handling: "none",
    run_worker_first: ["/api/*", "/artifacts/*"],
  },
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
      migrations_dir: fileURLToPath(
        new URL("../../apps/workers/migrations/d1", import.meta.url)
      ),
    },
  ],
  limits: { cpu_ms: input.cpu_ms },
  main: fileURLToPath(
    new URL("../../apps/workers/worker.mjs", import.meta.url)
  ),
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
  workers_dev: input.workers_dev === true,
};
writeFileSync(outputPath, `${JSON.stringify(config, null, 2)}\n`, {
  flag: "wx",
});
console.log(
  "Configuration written for review; no deployment performed. Entrypoint and migration paths are absolute."
);
