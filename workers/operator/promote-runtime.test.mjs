import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { Miniflare } from "miniflare";

const envelope = (revision, extra = "") =>
  JSON.stringify({
    catalog_id: "catalog",
    expires_at: 100,
    extra,
    revision,
  });

// This local-only fixture executes the production operation in workerd. Its
// verifier is intentionally a stub: Rust process tests own cryptographic proof.
test("workerd promotion preserves R2 bytes and conditionally advances real D1 rows", async () => {
  const runtime = new Miniflare({
    compatibilityDate: "2026-07-01",
    d1Databases: ["DB"],
    modules: [
      {
        contents: `import { promotePublication } from "./promote.mjs";
export default { async fetch(request, env) {
  const input = await request.json();
  try {
    return Response.json(await promotePublication({ ...input, database: env.DB,
      bucket: env.BUCKET, verify: JSON.parse, now: () => 50 }));
  } catch (error) { return Response.json({ error: error.message }, { status: 409 }); }
} };`,
        path: fileURLToPath(new URL("runtime-fixture.mjs", import.meta.url)),
        type: "ESModule",
      },
      {
        path: fileURLToPath(new URL("promote.mjs", import.meta.url)),
        type: "ESModule",
      },
    ],
    r2Buckets: ["BUCKET"],
  });
  try {
    const database = await runtime.getD1Database("DB");
    const bucket = await runtime.getR2Bucket("BUCKET");
    const migration = await readFile(
      new URL("../migrations/0001_public_reads.sql", import.meta.url),
      "utf-8"
    );
    await database.exec(
      migration.replaceAll(/--[^\n]*/gu, "").replaceAll("\n", " ")
    );
    await database
      .prepare("INSERT INTO marketplace_accepted VALUES(?,?,?)")
      .bind("catalog", "accepted-token", "accepted-object")
      .run();
    const promote = async (bytes, expected) => {
      const response = await runtime.dispatchFetch("http://fixture/promote", {
        body: JSON.stringify({
          catalogId: "catalog",
          envelope: bytes,
          expected,
        }),
        method: "POST",
      });
      return { body: await response.json(), status: response.status };
    };
    const first = await promote(envelope(1), null);
    assert.equal(first.status, 200);
    assert.equal(first.body.status, "published");
    const object = await bucket.get(first.body.object_key);
    assert.equal(await object.text(), envelope(1));
    assert.equal(
      await bucket.put(first.body.object_key, "overwrite", {
        onlyIf: { etagDoesNotMatch: "*" },
      }),
      null
    );
    const preserved = await bucket.get(first.body.object_key);
    assert.equal(await preserved.text(), envelope(1));
    const retry = await promote(envelope(1), null);
    assert.equal(retry.body.status, "already_published");
    const expected = {
      digest: first.body.digest,
      object_key: first.body.object_key,
      revision: 1,
    };
    const outcomes = await Promise.all([
      promote(envelope(2, "a"), expected),
      promote(envelope(2, "b"), expected),
    ]);
    assert.deepEqual(
      outcomes.map((result) => result.status).toSorted(),
      [200, 409]
    );
    const winner = outcomes.find((result) => result.status === 200).body;
    const pointer = await database
      .prepare(
        "SELECT revision,object_key,digest FROM marketplace_publications WHERE catalog_id=?"
      )
      .bind("catalog")
      .first();
    assert.deepEqual(pointer, {
      digest: winner.digest,
      object_key: winner.object_key,
      revision: 2,
    });
    const stale = await promote(envelope(3), expected);
    assert.equal(stale.status, 409);
    const accepted = await database
      .prepare("SELECT * FROM marketplace_accepted")
      .first();
    assert.deepEqual(accepted, {
      catalog_id: "catalog",
      object_key: "accepted-object",
      token: "accepted-token",
    });
  } finally {
    await runtime.dispose();
  }
});
