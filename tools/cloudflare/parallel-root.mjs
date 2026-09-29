import assert from "node:assert/strict";

// Read-only inventory of the legacy production root. Its signer and publisher
// database are unavailable; these identifiers must never be reused by v2.
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
