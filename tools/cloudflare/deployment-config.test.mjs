import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
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

const parallel = {
  ...valid,
  bucket_name: "lenso-marketplace-v2-production",
  catalog_id: "lenso-official-v2",
  database_id: "22222222-2222-4222-8222-222222222222",
  database_name: "lenso-marketplace-v2-production",
  deployment_track: "parallel-new-root",
  hostname: "marketplace-v2.lenso.dev",
  key_id: "lenso-marketplace-v2-2026",
  legacy_public_key_hex: "b".repeat(64),
  worker: "lenso-marketplace-v2",
};

const stableDomain = {
  ...valid,
  bucket_name: "lenso-marketplace-v2-production",
  catalog_id: "lenso-official-v2",
  database_id: "22222222-2222-4222-8222-222222222222",
  database_name: "lenso-marketplace-v2-production",
  deployment_track: "stable-domain-new-root",
  key_id: "lenso-marketplace-v2-2026",
  legacy_public_key_hex: "b".repeat(64),
  public_key_hex: "c".repeat(64),
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
  assert.equal(config.assets.binding, "ASSETS");
  assert.equal(config.assets.directory, join(root, "plugins/web/ui/dist"));
  assert.equal(config.assets.not_found_handling, "none");
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

test("routes signed document reads through the Worker before static assets", () => {
  const { outputPath, result } = run(valid);
  result();
  const config = JSON.parse(readFileSync(outputPath, "utf-8"));
  assert.deepEqual(config.assets.run_worker_first, [
    "/api/*",
    "/artifacts/*",
    "/documents/*",
  ]);
});

test("renders an isolated parallel root without copying migration metadata", () => {
  const { outputPath, result } = run(parallel);
  result();
  const config = JSON.parse(readFileSync(outputPath, "utf-8"));
  assert.equal(config.name, parallel.worker);
  assert.equal(config.routes[0].pattern, parallel.hostname);
  assert.equal(config.d1_databases[0].database_id, parallel.database_id);
  assert.equal(config.r2_buckets[0].bucket_name, parallel.bucket_name);
  assert.equal(config.vars.CATALOG_ID, parallel.catalog_id);
  assert.equal(config.vars.CATALOG_KEY_ID, parallel.key_id);
  assert.equal(config.vars.CATALOG_PUBLIC_KEY, parallel.public_key_hex);
  assert.ok(!Object.hasOwn(config, "legacy_public_key_hex"));
  assert.ok(!Object.hasOwn(config, "deployment_track"));
});

test("renders a stable-domain root with new trust and storage identities", () => {
  const { outputPath, result } = run(stableDomain);
  result();
  const config = JSON.parse(readFileSync(outputPath, "utf-8"));
  assert.equal(config.name, stableDomain.worker);
  assert.equal(config.routes[0].pattern, stableDomain.hostname);
  assert.equal(config.d1_databases[0].database_id, stableDomain.database_id);
  assert.equal(config.r2_buckets[0].bucket_name, stableDomain.bucket_name);
  assert.equal(config.vars.CATALOG_ID, stableDomain.catalog_id);
  assert.equal(config.vars.CATALOG_KEY_ID, stableDomain.key_id);
  assert.equal(config.vars.CATALOG_PUBLIC_KEY, stableDomain.public_key_hex);
  assert.ok(!Object.hasOwn(config, "legacy_public_key_hex"));
  assert.ok(!Object.hasOwn(config, "deployment_track"));
});

for (const [field, value] of [
  ["worker", "lenso-marketplace-v2"],
  ["hostname", "marketplace-v2.lenso.dev"],
]) {
  test(`stable-domain root refuses a changed canonical ${field}`, () => {
    const { result, outputPath } = run({ ...stableDomain, [field]: value });
    assert.throws(result);
    assert.equal(existsSync(outputPath), false);
  });
}

for (const field of [
  "database_name",
  "database_id",
  "bucket_name",
  "catalog_id",
  "key_id",
]) {
  test(`stable-domain root refuses the legacy ${field}`, () => {
    const input = { ...stableDomain, [field]: valid[field] };
    if (field === "database_id") {
      input.database_id = "cb599e1c-fb55-44f3-850b-679f2f934b67";
    }
    const { result, outputPath } = run(input);
    assert.throws(result);
    assert.equal(existsSync(outputPath), false);
  });
}

for (const field of [
  "worker",
  "hostname",
  "database_name",
  "database_id",
  "bucket_name",
  "catalog_id",
  "key_id",
]) {
  test(`parallel root refuses the legacy ${field}`, () => {
    const input = { ...parallel, [field]: valid[field] };
    if (field === "database_id") {
      input.database_id = "cb599e1c-fb55-44f3-850b-679f2f934b67";
    }
    const { result, outputPath } = run(input);
    assert.throws(result);
    assert.equal(existsSync(outputPath), false);
  });
}

test("parallel root refuses a reused signing public key", () => {
  const { result, outputPath } = run({
    ...parallel,
    legacy_public_key_hex: parallel.public_key_hex,
  });
  assert.throws(result);
  assert.equal(existsSync(outputPath), false);
});

test("parallel root requires a verified prior public key input", () => {
  const { result, outputPath } = run({
    ...parallel,
    legacy_public_key_hex: undefined,
  });
  assert.throws(result);
  assert.equal(existsSync(outputPath), false);
});

for (const [label, mutate] of [
  ["missing account", (input) => delete input.account_id],
  ["non-production environment", (input) => (input.environment = "proof")],
  ["unknown deployment track", (input) => (input.deployment_track = "other")],
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
