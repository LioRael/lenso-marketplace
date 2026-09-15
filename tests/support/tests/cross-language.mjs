import assert from "node:assert/strict";
// Independent signature-interoperability proof, not a production browser verifier.
import { createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";

const fixture = JSON.parse(
  readFileSync(new URL("conformance.json", import.meta.url), "utf-8")
);
const key = createPublicKey({
  format: "der",
  key: Buffer.concat([
    Buffer.from("302a300506032b6570032100", "hex"),
    Buffer.from(fixture.public_key_hex, "hex"),
  ]),
  type: "spki",
});
const { envelope } = fixture;
const payload = Buffer.from(envelope.payload_base64, "base64");
const message = Buffer.concat([
  Buffer.from("lenso.marketplace.snapshot.v1\0"),
  Buffer.from(envelope.key_id),
  Buffer.from([0]),
  payload,
]);
const signature = Buffer.from(envelope.signature_base64, "base64");
assert.ok(verify(null, message, key, signature));
assert.deepEqual(JSON.parse(payload), fixture.expected_payload);
message[message.length - 1] = (message.at(-1) + 1) % 256;
assert.equal(verify(null, message, key, signature), false);
console.log("Rust-to-Node Ed25519 signature and tamper rejection passed");
