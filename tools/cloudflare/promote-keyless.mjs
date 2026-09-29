import assert from "node:assert/strict";

import { validateCatalog } from "../keyless/catalog.mjs";
import { digest } from "../keyless/verify.mjs";

const columns =
  "catalog_id, revision, catalog_digest, catalog_size, bundle_digest, bundle_size, source_sha";

const readObject = async (object) => {
  assert.ok(object && object.size > 0 && object.size <= 4 * 1024 * 1024);
  const reader = object.body.getReader();
  const chunks = [];
  let size = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      size += value.byteLength;
      assert.ok(size <= object.size, "Stored object exceeds declared size");
      chunks.push(value);
    }
    assert.equal(size, object.size);
    return Buffer.concat(chunks);
  } finally {
    await reader.cancel();
    reader.releaseLock();
  }
};

export const promoteKeyless = async ({
  catalogBytes,
  bundleBytes,
  sourceSha,
  expected,
  verify,
  bucket,
  database,
}) => {
  for (const bytes of [catalogBytes, bundleBytes]) {
    assert.ok(
      Buffer.isBuffer(bytes) &&
        bytes.length > 0 &&
        bytes.length <= 4 * 1024 * 1024
    );
  }
  assert.match(sourceSha, /^[a-f0-9]{40}$/u);
  assert.ok(
    expected === null ||
      (expected &&
        Number.isSafeInteger(expected.revision) &&
        expected.revision > 0 &&
        /^[a-f0-9]{64}$/u.test(expected.catalog_digest))
  );
  const catalog = validateCatalog(JSON.parse(catalogBytes.toString("utf-8")));
  assert.ok(
    catalog.issued_at <= Math.floor(Date.now() / 1000) + 300,
    "Catalog issued in the future"
  );
  const catalogDigest = digest(catalogBytes);
  const bundleDigest = digest(bundleBytes);
  // The protected adapter must cryptographically verify these exact bytes.
  // Neither a caller-supplied receipt nor the catalog's own metadata is proof.
  await verify({ bundleBytes, catalogBytes, catalogDigest, sourceSha });
  const current = await database
    .prepare(
      `SELECT ${columns} FROM marketplace_keyless_heads WHERE catalog_id = ?`
    )
    .bind(catalog.catalog_id)
    .first();
  if (expected === null) {
    assert.equal(current, null, "Initial promotion requires an absent head");
  } else {
    assert.equal(
      current?.revision,
      expected.revision,
      "Head changed; reconcile before retrying"
    );
    assert.equal(
      current?.catalog_digest,
      expected.catalog_digest,
      "Head changed; reconcile before retrying"
    );
    assert.ok(catalog.revision > current.revision, "Revision must increase");
    const oldBytes = await readObject(
      await bucket.get(`keyless/${current.catalog_digest}.json`)
    );
    assert.equal(digest(oldBytes), current.catalog_digest);
    const old = validateCatalog(JSON.parse(oldBytes.toString("utf-8")));
    assert.equal(old.catalog_id, catalog.catalog_id);
    assert.equal(old.revision, current.revision);
    const next = new Map(
      catalog.releases.map((release) => [
        `${release.plugin_id}@${release.version}`,
        release,
      ])
    );
    for (const release of old.releases) {
      assert.deepEqual(
        next.get(`${release.plugin_id}@${release.version}`),
        release,
        "Published release identity is immutable; change status instead"
      );
    }
    const nextStatuses = new Map(
      catalog.statuses.map((status) => [
        `${status.plugin_id}@${status.version}`,
        status,
      ])
    );
    for (const status of old.statuses) {
      if (status.state === "revoked") {
        assert.equal(
          nextStatuses.get(`${status.plugin_id}@${status.version}`)?.state,
          "revoked",
          "Revocation is terminal for an exact release version"
        );
      }
    }
  }
  for (const bytes of [catalogBytes, bundleBytes]) {
    const sha = digest(bytes);
    const key = `keyless/${sha}.json`;
    const written = await bucket.put(key, bytes, {
      onlyIf: { etagDoesNotMatch: "*" },
    });
    if (!written) {
      assert.equal(
        digest(await readObject(await bucket.get(key))),
        sha,
        "Existing immutable object mismatch"
      );
    }
  }
  const values = [
    catalog.revision,
    catalogDigest,
    catalogBytes.length,
    bundleDigest,
    bundleBytes.length,
    sourceSha,
    catalog.catalog_id,
  ];
  const sql =
    expected === null
      ? `INSERT INTO marketplace_keyless_heads (${columns}) SELECT ?, ?, ?, ?, ?, ?, ? WHERE NOT EXISTS (SELECT 1 FROM marketplace_keyless_heads WHERE catalog_id = ?)`
      : "UPDATE marketplace_keyless_heads SET revision = ?, catalog_digest = ?, catalog_size = ?, bundle_digest = ?, bundle_size = ?, source_sha = ? WHERE catalog_id = ? AND revision = ? AND catalog_digest = ?";
  const params =
    expected === null
      ? [catalog.catalog_id, ...values.slice(0, 6), catalog.catalog_id]
      : [...values, expected.revision, expected.catalog_digest];
  const result = await database
    .prepare(sql)
    .bind(...params)
    .run();
  assert.equal(
    result.meta?.changes,
    1,
    "Promotion unconfirmed; inspect current head before retrying"
  );
  const receipt = await database
    .prepare(
      `SELECT ${columns} FROM marketplace_keyless_heads WHERE catalog_id = ?`
    )
    .bind(catalog.catalog_id)
    .first();
  assert.deepEqual(
    receipt,
    {
      bundle_digest: bundleDigest,
      bundle_size: bundleBytes.length,
      catalog_digest: catalogDigest,
      catalog_id: catalog.catalog_id,
      catalog_size: catalogBytes.length,
      revision: catalog.revision,
      source_sha: sourceSha,
    },
    "Head receipt differs; reconcile before retrying"
  );
  return receipt;
};
