// Explicitly mutates only the disposable G3 accepted pointer through its guarded
// adapter proof. Never resets state, changes publication, or retries a POST.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [base, fixturePath, location, outputPath] = process.argv.slice(2);
assert.ok(
  base &&
    fixturePath &&
    outputPath &&
    ["--local", "--remote"].includes(location),
  "usage: node proof/consumers-race.mjs BASE RACE_JSON --local|--remote NEW_RECEIPT_JSON"
);
const url = new URL(base);
assert.ok(
  !url.username &&
    !url.password &&
    url.pathname === "/" &&
    !url.search &&
    !url.hash,
  "base must be an origin"
);
if (location === "--remote") {
  assert.equal(
    url.origin,
    "https://lenso-marketplace-g3-proof.lenso.workers.dev",
    "only G3 proof origin allowed"
  );
} else {
  assert.ok(
    url.protocol === "http:" &&
      ["127.0.0.1", "localhost"].includes(url.hostname),
    "only explicit localhost allowed"
  );
}

const root = fileURLToPath(new URL("..", import.meta.url));
const config = JSON.parse(
  readFileSync(resolve(root, "wrangler.jsonc"), "utf-8")
);
const database = "2b913921-3f0a-43b4-ae8f-cb219c21db81";
const resource = "lenso-marketplace-g3-proof";
const catalog = "workers-g3-proof";
assert.equal(config.name, resource);
assert.equal(
  config.main,
  "proof-worker.mjs",
  "proof wrapper must be explicitly deployed"
);
assert.equal(config.vars.CATALOG_ID, catalog);
assert.equal(config.vars.G3_PROOF_DATABASE_ID, database);
assert.equal(config.vars.G3_PROOF_BUCKET_NAME, resource);
assert.equal(
  config.d1_databases.find((item) => item.binding === "MARKETPLACE_DB")
    ?.database_id,
  database
);
assert.equal(
  config.d1_databases.find((item) => item.binding === "MARKETPLACE_DB")
    ?.database_name,
  resource
);
assert.equal(
  config.r2_buckets.find((item) => item.binding === "MARKETPLACE_OBJECTS")
    ?.bucket_name,
  resource
);
const secret = process.env.G3_PROOF_KEY;
assert.ok(
  typeof secret === "string" && secret.length >= 32 && secret.length <= 256,
  "supply G3_PROOF_KEY through the environment"
);
const bytes = readFileSync(fixturePath);
assert.ok(bytes.length <= 128 * 1024, "fixture exceeds proof input bound");
const fixture = JSON.parse(bytes);
const hash = (value) => createHash("sha256").update(value).digest("hex");
assert.equal(
  config.vars.G3_CAS_FIXTURE_SHA256,
  hash(bytes),
  "deployed fixture digest must be pinned"
);
assert.equal(fixture.catalog, catalog);
assert.equal(fixture.expected.checkpoint.revision, 5);
assert.deepEqual(
  fixture.candidates.map((item) => item.checkpoint.revision),
  [6, 7]
);
assert.ok(
  Math.floor(Date.now() / 1000) >= fixture.verified_at &&
    Math.floor(Date.now() / 1000) < fixture.valid_until,
  "Rust verification receipt expired"
);

const wrangler = (...args) =>
  execFileSync("pnpm", ["exec", "wrangler", ...args], {
    cwd: root,
    encoding: "utf-8",
    maxBuffer: 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  });
const query = (sql) => {
  const result = JSON.parse(
    wrangler("d1", "execute", resource, location, "--command", sql, "--json")
  );
  assert.equal(result.length, 1);
  assert.equal(result[0].success, true);
  return result[0].results;
};
const readDurable = () => {
  const rows = query(
    "SELECT catalog_id,token,object_key FROM marketplace_accepted WHERE catalog_id='workers-g3-proof'"
  );
  assert.equal(rows.length, 1, "expected existing accepted state");
  const [pointer] = rows;
  assert.ok(
    /^accepted\/workers-g3-proof\/[0-9a-f]{64}\.json$/u.test(
      pointer.object_key
    ),
    "unexpected object key"
  );
  const object = wrangler(
    "r2",
    "object",
    "get",
    `${resource}/${pointer.object_key}`,
    location,
    "--pipe"
  );
  assert.equal(
    pointer.object_key,
    `accepted/${catalog}/${hash(object)}.json`,
    "whole checkpoint integrity"
  );
  const state = JSON.parse(object);
  assert.equal(state.token, pointer.token);
  assert.equal(state.token, `sha256:${hash(state.envelope)}`);
  return {
    object_bytes: Buffer.byteLength(object),
    object_sha256: hash(object),
    pointer,
    state,
  };
};
const publicRows = query(
  "SELECT revision,object_key,digest FROM marketplace_publications WHERE catalog_id='workers-g3-proof'"
);
assert.equal(publicRows.length, 1);
assert.equal(
  publicRows[0].revision,
  5,
  "full smoke must first finish at restored revision 5"
);
assert.equal(publicRows[0].digest, fixture.expected.token);
const before = readDurable();
assert.deepEqual(
  before.state,
  fixture.expected,
  "accepted state must match the Rust-verified history before any write"
);
// Reserve a new receipt path before mutation; refuse to overwrite prior evidence.
writeFileSync(
  outputPath,
  `${JSON.stringify(
    { before, fixture_sha256: hash(bytes), passed: false, phase: "prepared" },
    null,
    2
  )}\n`,
  { flag: "wx" }
);
let body, response;
try {
  response = await fetch(new URL("/__proof/cas", url), {
    body: bytes,
    headers: {
      authorization: `Bearer ${secret}`,
      "content-type": "application/json",
    },
    method: "POST",
    redirect: "error",
    signal: AbortSignal.timeout(15000),
  });
  body = await response.json();
} catch {
  // Preserve the prepared receipt; never replay an ambiguous write.
  throw new Error(
    "CAS request uncertain; inspect durable proof state before any retry"
  );
}
const after = readDurable();
const receipt = {
  after,
  base: url.origin,
  before,
  fixture_sha256: hash(bytes),
  location,
  passed: false,
  publication_before: publicRows[0],
  result: body,
  status: response.status,
  timestamp: new Date().toISOString(),
};
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`);
assert.equal(
  response.status,
  200,
  "proof did not return a confirmed result; inspect receipt before retrying"
);
assert.equal(body.passed, true);
assert.deepEqual([...body.results].toSorted(), [false, true]);
assert.equal(body.stale_result, false);
assert.equal(body.expected_token, fixture.expected.token);
assert.equal(body.fixture_sha256, hash(bytes));
assert.deepEqual(
  after.state,
  fixture.candidates[body.winner],
  "independent D1/R2 read must equal the verified winning candidate"
);
assert.deepEqual(body.after.pointer, after.pointer);
assert.deepEqual(body.final.pointer, after.pointer);
assert.equal(body.before.history_size, 161);
assert.equal(body.after.history_sha256, body.before.history_sha256);
assert.equal(body.final.history_sha256, body.before.history_sha256);
receipt.passed = true;
writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(
  JSON.stringify({
    passed: true,
    receipt: resolve(outputPath),
    results: body.results,
    stale_result: body.stale_result,
    winner_revision: after.state.checkpoint.revision,
  })
);
