import assert from "node:assert/strict";
import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";

export const catalogId = "workers-d01-local-only";
export const keyId = "d01-public-fixture";
export const sha256 = (bytes) =>
  createHash("sha256").update(bytes).digest("hex");

// This recipe and the committed signatures reconstruct exact public bytes.
// No signing key is needed or retained. Expired catalogs remain valid for browse.
const ordered = (...fields) => Object.fromEntries(fields);
export const payload = (size, revision, issued = 100) =>
  JSON.stringify(
    ordered(
      ["schema", "lenso.marketplace.snapshot.v1"],
      ["catalog_id", catalogId],
      ["revision", revision],
      ["issued_at", issued],
      ["expires_at", 200],
      [
        "releases",
        Array.from({ length: size === "small" ? 1 : 161 }, (_, index) =>
          ordered(
            ["plugin_id", `lenso.d01.p${String(index).padStart(3, "0")}`],
            ["version", "0.1.0"],
            ["publisher_id", "lenso"],
            ["title", `D01 ${String(index).padStart(3, "0")}`],
            ["summary", "Local characterization fixture"],
            ...(index === 0 ? [] : [["description", "x".repeat(16 * 1024)]]),
            ["source_url", "https://example.test/source"],
            ["source_revision", "a".repeat(40)],
            ["license", "MIT"],
            [
              "artifact",
              ordered(
                ["url", "https://example.test/fixture.lenso-plugin"],
                ["digest", `sha256:${sha256("archive")}`],
                ["size", 7],
                ["manifest_digest", `sha256:${sha256("manifest")}`]
              ),
            ],
            ["availability", "listed"]
          )
        ),
      ]
    )
  );

export const fixtures = () => {
  const manifest = JSON.parse(
    readFileSync(new URL("signatures.json", import.meta.url), "utf-8")
  );
  const publicKey = createPublicKey({
    format: "der",
    key: Buffer.from(manifest.publicKeyDerBase64, "base64"),
    type: "spki",
  });
  const entries = {};
  for (const [name, record] of Object.entries(manifest.envelopes)) {
    const bytes = Buffer.from(
      payload(record.size, record.revision, record.issued)
    );
    const envelope = JSON.stringify({
      key_id: keyId,
      payload_base64: bytes.toString("base64"),
      signature_base64: record.signatureBase64,
    });
    assert.ok(
      verify(
        null,
        Buffer.concat([
          Buffer.from(`lenso.marketplace.snapshot.v1\0${keyId}\0`),
          bytes,
        ]),
        publicKey,
        Buffer.from(record.signatureBase64, "base64")
      ),
      `${name}: public signature`
    );
    assert.equal(sha256(envelope), record.sha256, `${name}: exact bytes`);
    assert.equal(Buffer.byteLength(envelope), record.bytes);
    entries[name] = {
      digest: `sha256:${record.sha256}`,
      envelope,
      object_key: `publications/${name}.json`,
      snapshot: JSON.parse(bytes),
      ...record,
    };
  }
  return { entries, publicKeyHex: manifest.publicKeyHex };
};
