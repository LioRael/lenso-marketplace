import assert from "node:assert/strict";
import { test } from "node:test";

import { verifyLegacyEnvelope } from "./migrate.mjs";

test("legacy migration rejects unsigned, wrong-key and noncanonical envelopes", () => {
  const values = [
    { releases: [] },
    { key_id: "other-key", payload_base64: "e30=", signature_base64: "AA==" },
    {
      key_id: "lenso-marketplace-v2-2026",
      payload_base64: "e30",
      signature_base64: "AA==",
    },
    {
      key_id: "lenso-marketplace-v2-2026",
      payload_base64: "e30=",
      signature_base64: Buffer.alloc(64).toString("base64"),
    },
  ];
  for (const value of values) {
    assert.throws(() =>
      verifyLegacyEnvelope(Buffer.from(JSON.stringify(value)), "portable", 1)
    );
  }
});
