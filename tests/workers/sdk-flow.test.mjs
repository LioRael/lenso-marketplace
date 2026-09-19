import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { Miniflare } from "miniflare";

import { catalogId, fixtures } from "./profile/fixtures.mjs";

const configPath = new URL(
  "../../apps/workers-sdk-prototype/wrangler.jsonc",
  import.meta.url
);
const cargoPath = new URL(
  "../../apps/workers-sdk-prototype/Cargo.toml",
  import.meta.url
);
const buildPath = new URL(
  "../../apps/workers-sdk-prototype/build.sh",
  import.meta.url
);

test("Workers SDK prototype pins its toolchain and source watch inputs", async () => {
  const [config, cargo, build] = await Promise.all([
    readFile(configPath, "utf-8"),
    readFile(cargoPath, "utf-8"),
    readFile(buildPath, "utf-8"),
  ]);
  assert.match(cargo, /worker\s*=\s*\{\s*version\s*=\s*"=0\.8\.5"/u);
  assert.match(build, /--version/u);
  assert.match(build, /0\.8\.5/u);
  for (const path of [
    '"src"',
    '"../../plugins/directory/src"',
    '"../../plugins/web/src"',
    '"../../contracts/directory/src"',
  ]) {
    assert.match(
      config,
      new RegExp(path.replaceAll(/[.*+?^${}()|[\]\\]/gu, "\\$&"), "u")
    );
  }
  assert.match(config, /build\/worker\/index\.js/u);
  assert.match(config, /sdk-prototype-local/u);
  assert.match(config, /workers-d01-local-only/u);
  assert.match(config, /d01-public-fixture/u);
});

test(
  "Workers SDK prototype serves a signed browse request through local D1 and R2",
  { skip: process.env.RUN_WORKERS_SDK_FLOW !== "1" },
  async () => {
    const { entries, publicKeyHex } = fixtures();
    const entry = entries["small-target"];
    const temporary = await mkdtemp(join(tmpdir(), "lenso-marketplace-sdk-"));
    const artifact = fileURLToPath(
      new URL("../../apps/workers-sdk-prototype/build/worker/", import.meta.url)
    );
    const runtime = new Miniflare({
      bindings: {
        CATALOG_ID: catalogId,
        CATALOG_KEY_ID: "d01-public-fixture",
        CATALOG_PUBLIC_KEY: publicKeyHex,
      },
      compatibilityDate: "2026-07-08",
      compatibilityFlags: [
        "global_fetch_strictly_public",
        "enable_request_signal",
      ],
      d1Databases: ["MARKETPLACE_DB"],
      d1Persist: join(temporary, "d1"),
      host: "127.0.0.1",
      modules: true,
      modulesRoot: artifact,
      modulesRules: [{ include: ["**/*.wasm"], type: "CompiledWasm" }],
      outboundService: () =>
        new Response("Outbound access forbidden", { status: 502 }),
      port: 0,
      r2Buckets: ["MARKETPLACE_OBJECTS"],
      r2Persist: join(temporary, "r2"),
      scriptPath: join(artifact, "index.js"),
    });
    try {
      await runtime.ready;
      const migration = await readFile(
        new URL(
          "../../apps/workers/migrations/d1/0001_public_reads.sql",
          import.meta.url
        ),
        "utf-8"
      );
      const database = await runtime.getD1Database("MARKETPLACE_DB");
      const bucket = await runtime.getR2Bucket("MARKETPLACE_OBJECTS");
      await database.exec(
        migration.replaceAll(/--[^\n]*/gu, " ").replaceAll("\n", " ")
      );
      await bucket.put(entry.object_key, entry.envelope);
      await database
        .prepare("INSERT INTO marketplace_publications VALUES(?,?,?,?)")
        .bind(catalogId, entry.revision, entry.object_key, entry.digest)
        .run();

      const response = await fetch(
        new URL("/api/marketplace/v1/plugins?limit=1", await runtime.ready)
      );
      assert.equal(response.status, 200);
      const body = await response.json();
      assert.equal(body.catalog_id, catalogId);
      assert.equal(body.revision, entry.revision);
      assert.equal(body.total, entry.snapshot.releases.length);
      assert.deepEqual(body.releases, [entry.snapshot.releases[0]]);

      // Publish a newer signed envelope through the same durable fixture. The
      // second request exercises the existing-pointer UPDATE path, including
      // its expected token/object-key fence.
      const newer = entries["small-newer"];
      await bucket.put(newer.object_key, newer.envelope);
      await database
        .prepare(
          "INSERT OR REPLACE INTO marketplace_publications VALUES(?,?,?,?)"
        )
        .bind(catalogId, newer.revision, newer.object_key, newer.digest)
        .run();
      const updatedResponse = await fetch(
        new URL("/api/marketplace/v1/plugins?limit=1", await runtime.ready)
      );
      assert.equal(updatedResponse.status, 200);
      const updated = await updatedResponse.json();
      assert.equal(updated.catalog_id, catalogId);
      assert.equal(updated.revision, newer.revision);
      assert.equal(updated.total, newer.snapshot.releases.length);
      assert.deepEqual(updated.releases, [newer.snapshot.releases[0]]);
    } finally {
      await runtime.dispose();
    }
  }
);
