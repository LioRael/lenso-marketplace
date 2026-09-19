import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { catalogId, fixtures } from "../tests/workers/profile/fixtures.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const config = "apps/workers-sdk-prototype/wrangler.jsonc";
const bucket = "lenso-marketplace-sdk-prototype-local";
const database = "MARKETPLACE_DB";
const entry = fixtures().entries["small-target"];
const temporary = await mkdtemp(join(tmpdir(), "lenso-marketplace-sdk-seed-"));
const envelope = join(temporary, "publication.json");
const migration = join(
  root,
  "apps/workers/migrations/d1/0001_public_reads.sql"
);
const migrationContents = await readFile(migration, "utf-8");
const schema = migrationContents
  .replaceAll("CREATE TABLE ", "CREATE TABLE IF NOT EXISTS ")
  .replaceAll(/--[^\n]*/gu, " ");

const wrangler = (args) =>
  execFileSync("pnpm", ["exec", "wrangler", ...args], {
    cwd: root,
    stdio: "inherit",
  });
const sqlString = (value) => `'${value.replaceAll("'", "''")}'`;

try {
  await writeFile(envelope, entry.envelope);
  wrangler([
    "d1",
    "execute",
    database,
    "--local",
    "--config",
    config,
    "--command",
    schema,
  ]);
  wrangler([
    "r2",
    "object",
    "put",
    `${bucket}/${entry.object_key}`,
    "--local",
    "--config",
    config,
    "--file",
    envelope,
  ]);
  wrangler([
    "d1",
    "execute",
    database,
    "--local",
    "--config",
    config,
    "--command",
    [
      "INSERT OR REPLACE INTO marketplace_publications",
      "(catalog_id,revision,object_key,digest) VALUES",
      `(${sqlString(catalogId)},${entry.revision},${sqlString(entry.object_key)},${sqlString(entry.digest)})`,
    ].join(" "),
  ]);
  console.log(
    `Seeded ${catalogId} revision ${entry.revision} into local Wrangler D1/R2 state.`
  );
} finally {
  await rm(temporary, { force: true, recursive: true });
}
