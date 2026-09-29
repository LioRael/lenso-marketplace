import assert from "node:assert/strict";

// Read-only inventory of the retired production trust material. Its signer and
// publisher database are unavailable. The resource identifiers are collision
// guards for a new trust root; the public Worker and hostname are intentionally
// reusable for a stable-domain cutover.
const legacy = Object.freeze({
  bucket_name: "lenso-marketplace-production",
  catalog_id: "lenso-official",
  database_id: "cb599e1c-fb55-44f3-850b-679f2f934b67",
  database_name: "lenso-marketplace-production",
  hostname: "marketplace.lenso.dev",
  key_id: "lenso-marketplace-2026",
  worker: "lenso-marketplace",
});

export const assertParallelRoot = (input) => {
  assert.equal(
    input.deployment_track,
    "parallel-new-root",
    "parallel root requires an explicit deployment track"
  );
  for (const [field, previous] of Object.entries(legacy)) {
    assert.ok(
      typeof input[field] === "string" && input[field].length > 0,
      `parallel root requires ${field}`
    );
    assert.notEqual(
      input[field],
      previous,
      `parallel root reuses legacy ${field}`
    );
  }
  assert.match(
    input.legacy_public_key_hex,
    /^[a-f0-9]{64}$/u,
    "parallel root requires the verified legacy public key"
  );
  assert.match(input.public_key_hex, /^[a-f0-9]{64}$/u);
  assert.notEqual(
    input.public_key_hex,
    input.legacy_public_key_hex,
    "parallel root reuses the legacy signing key"
  );
};

// A stable-domain cutover keeps the public service identity while replacing
// every production trust/storage identity behind it. This is intentionally a
// separate track from a parallel origin: callers keep using the canonical
// marketplace hostname, while the old catalog, D1, R2 and signing key are not
// reused.
export const assertStableDomainRoot = (input) => {
  assert.equal(
    input.deployment_track,
    "stable-domain-new-root",
    "stable-domain root requires an explicit deployment track"
  );
  assert.equal(
    input.worker,
    legacy.worker,
    "stable-domain root must keep the canonical Worker"
  );
  assert.equal(
    input.hostname,
    legacy.hostname,
    "stable-domain root must keep the canonical hostname"
  );
  for (const field of [
    "bucket_name",
    "catalog_id",
    "database_id",
    "database_name",
    "key_id",
  ]) {
    assert.ok(
      typeof input[field] === "string" && input[field].length > 0,
      `stable-domain root requires ${field}`
    );
    assert.notEqual(
      input[field],
      legacy[field],
      `stable-domain root reuses legacy ${field}`
    );
  }
  assert.match(
    input.legacy_public_key_hex,
    /^[a-f0-9]{64}$/u,
    "stable-domain root requires the verified legacy public key"
  );
  assert.match(input.public_key_hex, /^[a-f0-9]{64}$/u);
  assert.notEqual(
    input.public_key_hex,
    input.legacy_public_key_hex,
    "stable-domain root reuses the legacy signing key"
  );
};
