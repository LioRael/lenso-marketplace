import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { Miniflare } from "miniflare";

const envelope = (revision, extra = "") =>
  JSON.stringify({ catalog_id: "catalog", expires_at: 100, extra, revision });

test("workerd release content uses an independent D1 pointer and immutable R2 bytes", async () => {
  const runtime = new Miniflare({
    compatibilityDate: "2026-07-01",
    d1Databases: ["DB"],
    modules: [
      {
        contents: `import { promoteReleaseContent } from "./promote.mjs";
export default { async fetch(request, env) {
  const input = await request.json();
  try {
    return Response.json(await promoteReleaseContent({ ...input, database: env.DB,
      bucket: env.BUCKET, verify: JSON.parse, now: () => 50 }));
  } catch (error) { return Response.json({ error: error.message }, { status: 409 }); }
} };`,
        path: fileURLToPath(
          new URL("release-content-fixture.mjs", import.meta.url)
        ),
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
    for (const name of ["0001_public_reads.sql", "0004_release_content.sql"]) {
      const migration = await readFile(
        new URL(`../../apps/workers/migrations/d1/${name}`, import.meta.url),
        "utf-8"
      );
      await database.exec(
        migration.replaceAll(/--[^\n]*/gu, "").replaceAll("\n", " ")
      );
    }
    await database
      .prepare("INSERT INTO marketplace_publications VALUES(?,?,?,?)")
      .bind("catalog", 7, "publications/old.json", "sha256:old")
      .run();
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
    assert.match(first.body.object_key, /^release-content\//u);
    const firstObject = await bucket.get(first.body.object_key);
    assert.equal(await firstObject.text(), envelope(1));
    const retry = await promote(envelope(1), null);
    assert.equal(retry.body.status, "already_published");
    const expected = {
      digest: first.body.digest,
      object_key: first.body.object_key,
      revision: 1,
    };
    const contenders = [envelope(2, "a"), envelope(2, "b")];
    const outcomes = await Promise.all(
      contenders.map((bytes) => promote(bytes, expected))
    );
    assert.deepEqual(
      outcomes.map((result) => result.status).toSorted(),
      [200, 409]
    );
    const winner = outcomes.findIndex((result) => result.status === 200);
    const pointer = await database
      .prepare(
        "SELECT revision,object_key,digest FROM marketplace_release_content WHERE catalog_id=?"
      )
      .bind("catalog")
      .first();
    assert.deepEqual(pointer, {
      digest: outcomes[winner].body.digest,
      object_key: outcomes[winner].body.object_key,
      revision: 2,
    });
    const winnerObject = await bucket.get(pointer.object_key);
    assert.equal(await winnerObject.text(), contenders[winner]);
    const stale = await promote(envelope(3), expected);
    assert.equal(stale.status, 409);
    assert.deepEqual(
      await database.prepare("SELECT * FROM marketplace_publications").first(),
      {
        catalog_id: "catalog",
        digest: "sha256:old",
        object_key: "publications/old.json",
        revision: 7,
      }
    );
    assert.deepEqual(
      await database.prepare("SELECT * FROM marketplace_accepted").first(),
      {
        catalog_id: "catalog",
        object_key: "accepted-object",
        token: "accepted-token",
      }
    );
  } finally {
    await runtime.dispose();
  }
});
