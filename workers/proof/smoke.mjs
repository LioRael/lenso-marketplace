import assert from "node:assert/strict";
// Only operates on the dedicated G3 proof database/bucket, never production.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const base = process.argv[2] ?? "http://127.0.0.1:63735";
const location = process.argv[3] ?? "--local";
assert.ok(["--local", "--remote"].includes(location));
const fixtures = process.argv[4] ?? "/tmp/lenso-workers-g3-fixtures";
const temporary = mkdtempSync(join(tmpdir(), "lenso-g3-proof-"));
const resource = "lenso-marketplace-g3-proof";
const records = [];
const wrangler = (...args) =>
  execFileSync("pnpm", ["exec", "wrangler", ...args], {
    encoding: "utf-8",
    stdio: ["ignore", "pipe", "pipe"],
  });
// Small fixture mutations use the query API, not a bulk database import.
const sql = (text) => {
  wrangler("d1", "execute", resource, location, "--command", text);
};
const select = (name, revision = 1) => {
  const file = join(fixtures, `${name}.json`);
  const digest = `sha256:${createHash("sha256").update(readFileSync(file)).digest("hex")}`;
  wrangler(
    "r2",
    "object",
    "put",
    `${resource}/proof/${name}.json`,
    "--file",
    file,
    location
  );
  sql(
    `INSERT INTO marketplace_publications VALUES('workers-g3-proof',${revision},'proof/${name}.json','${digest}') ON CONFLICT(catalog_id) DO UPDATE SET revision=excluded.revision,object_key=excluded.object_key,digest=excluded.digest;`
  );
};
const check = async (label, status, path = "/api/marketplace/v1/plugins") => {
  const response = await fetch(base + path, {
    headers: { connection: "close" },
    signal: AbortSignal.timeout(15000),
  });
  const text = await response.text();
  assert.equal(response.status, status, `${label}: ${text.slice(0, 200)}`);
  records.push({
    boot: response.headers.get("x-proof-boot"),
    generation: response.headers.get("x-proof-generation"),
    label,
    status,
    wasmMemoryBytes: Number(response.headers.get("x-proof-wasm-memory")),
  });
  return { response, text };
};
sql(
  "DELETE FROM marketplace_accepted WHERE catalog_id='workers-g3-proof'; DELETE FROM marketplace_publications WHERE catalog_id='workers-g3-proof';"
);
await check("not published", 503);
select("first");
const first = await check("verified published snapshot", 200);
assert.equal(
  JSON.parse(first.text).releases[0].plugin_id,
  "lenso.marketplace.echo"
);
const raw = await check(
  "exact signed bytes",
  200,
  "/api/marketplace/v1/snapshot"
);
assert.equal(raw.text, readFileSync(join(fixtures, "first.json"), "utf-8"));
const concurrent = await Promise.all(
  Array.from({ length: 12 }, async (_, i) => {
    const response = await fetch(`${base}/api/marketplace/v1/plugins`, {
      headers: { connection: "close" },
      signal: AbortSignal.timeout(15000),
    });
    await response.text();
    assert.ok(
      [200, 503].includes(response.status),
      "unexpected admission status"
    );
    records.push({ label: `bounded admission ${i}`, status: response.status });
    return response.status;
  })
);
assert.ok(concurrent.includes(200), "no admitted request succeeded");
select("equivocation");
await check("same revision equivocation", 503);
select("tampered");
await check("invalid signature", 503);
select("expired", 3);
await check("expired snapshot", 503);
select("second", 2);
await check("new revision", 200);
const [
  {
    results: [pointer],
  },
] = JSON.parse(
  wrangler(
    "d1",
    "execute",
    resource,
    location,
    "--command",
    "SELECT object_key FROM marketplace_accepted WHERE catalog_id='workers-g3-proof'",
    "--json"
  )
);
const saved = join(temporary, "accepted-original.json");
wrangler(
  "r2",
  "object",
  "get",
  `${resource}/${pointer.object_key}`,
  "--file",
  saved,
  location
);
const corrupted = JSON.parse(readFileSync(saved, "utf-8"));
assert.ok(
  Object.keys(corrupted.checkpoint.release_identities).length > 0,
  "history vector must be populated"
);
corrupted.checkpoint.release_identities = {};
const corruptFile = join(temporary, "accepted-corrupt.json");
writeFileSync(corruptFile, JSON.stringify(corrupted));
wrangler(
  "r2",
  "object",
  "put",
  `${resource}/${pointer.object_key}`,
  "--file",
  corruptFile,
  location
);
await check("checkpoint object tampering", 503);
wrangler(
  "r2",
  "object",
  "put",
  `${resource}/${pointer.object_key}`,
  "--file",
  saved,
  location
);
select("identity-change", 3);
await check("removed identity cannot change artifact", 503);

select("first");
await check("rollback", 503);
select("second", 2);
sql(
  "UPDATE marketplace_publications SET object_key='proof/missing.json' WHERE catalog_id='workers-g3-proof';"
);
await check("storage outage fails closed", 503);
select("first");
await check("outage preserves rollback fence", 503);
select("second", 2);
for (let i = 0; i < 100; i += 1) {
  await check(`recreation ${i}`, 200);
}
select("large", 4);
await check("large signed catalog verification", 200);
const large = await check(
  "large signed snapshot response",
  200,
  "/api/marketplace/v1/snapshot"
);
assert.equal(large.text, readFileSync(join(fixtures, "large.json"), "utf-8"));
select("restored", 5);
await check("recovery after large catalog", 200);
const boots = new Map();
for (const r of records) {
  if (r.boot && r.generation) {
    if (!boots.has(r.boot)) {
      boots.set(r.boot, new Set());
    }
    boots.get(r.boot).add(r.generation);
  }
}
assert.ok(
  [...boots.values()].some((g) => g.size >= 2),
  "did not observe persistent reads across same-isolate Wasm recreation"
);
console.log(
  JSON.stringify(
    {
      base,
      checks: records.length,
      location,
      passed: true,
      records,
      recreatedBoots: [...boots]
        .filter(([, v]) => v.size >= 2)
        .map(([boot, values]) => ({ boot, generations: [...values] })),
    },
    null,
    2
  )
);
