import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const script = join(root, "tools/cloudflare/deployment-config.mjs");
const productionKey = "a".repeat(64);

const valid = {
  account_id: "05ac654d4a24ee5ef1d683440c8c0068",
  bucket_name: "lenso-marketplace-production",
  catalog_id: "lenso-official",
  cpu_ms: 1000,
  database_id: "11111111-1111-4111-8111-111111111111",
  database_name: "lenso-marketplace-production",
  environment: "production",
  hostname: "marketplace.lenso.dev",
  key_id: "lenso-marketplace-2026",
  public_key_hex: productionKey,
  worker: "lenso-marketplace",
};

const run = (input) => {
  const directory = mkdtempSync(join(tmpdir(), "lenso-marketplace-config-"));
  const inputPath = join(directory, "input.json");
  const outputPath = join(directory, "wrangler.json");
  writeFileSync(inputPath, JSON.stringify(input));
  const result = () =>
    execFileSync(process.execPath, [script, inputPath, outputPath], {
      cwd: root,
      encoding: "utf-8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  return { outputPath, result };
};

test("renders an account-bound production Worker config", () => {
  const { result, outputPath } = run(valid);
  assert.match(result(), /Configuration written for review/u);
  const config = JSON.parse(readFileSync(outputPath, "utf-8"));
  assert.equal(config.account_id, valid.account_id);
  assert.equal(config.name, valid.worker);
  assert.deepEqual(config.routes, [
    { custom_domain: true, pattern: valid.hostname },
  ]);
  assert.equal(config.d1_databases[0].database_id, valid.database_id);
  assert.equal(config.r2_buckets[0].bucket_name, valid.bucket_name);
  assert.deepEqual(config.assets, {
    binding: "ASSETS",
    directory: join(root, "plugins/web/ui/dist"),
    not_found_handling: "none",
    run_worker_first: ["/api/*", "/artifacts/*"],
  });
  assert.deepEqual(config.vars, {
    CATALOG_ID: valid.catalog_id,
    CATALOG_KEY_ID: valid.key_id,
    CATALOG_PUBLIC_KEY: valid.public_key_hex,
  });
  assert.equal(config.workers_dev, false);
});

test("can explicitly retain the public Agent workers.dev origin", () => {
  const { outputPath, result } = run({ ...valid, workers_dev: true });
  assert.match(result(), /Configuration written for review/u);
  assert.equal(JSON.parse(readFileSync(outputPath, "utf-8")).workers_dev, true);
});

for (const [label, mutate] of [
  ["missing account", (input) => delete input.account_id],
  ["non-production environment", (input) => (input.environment = "proof")],
  [
    "workers.dev hostname",
    (input) => (input.hostname = "lenso-marketplace.workers.dev"),
  ],
  [
    "proof resource",
    (input) => (input.database_name = "lenso-marketplace-g3-proof"),
  ],
  [
    "recovery resource",
    (input) => (input.bucket_name = "lenso-marketplace-recovery"),
  ],
  [
    "known proof database",
    (input) => (input.database_id = "2b913921-3f0a-43b4-ae8f-cb219c21db81"),
  ],
  [
    "publisher proof key",
    (input) =>
      (input.public_key_hex =
        "8d47ca04c5564c517371edc4540ee0945e4b15c3843b6a04a753d11b257b27c9"),
  ],
]) {
  test(`rejects ${label}`, () => {
    const input = structuredClone(valid);
    mutate(input);
    const { result } = run(input);
    assert.throws(result);
  });
}
