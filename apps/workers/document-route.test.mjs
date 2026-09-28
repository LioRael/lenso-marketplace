import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import { documentResponse } from "./documents.mjs";

const body = new TextEncoder().encode("# A signed, exact guide\n");
const documentDigest = `sha256:${createHash("sha256").update(body).digest("hex")}`;
const path = `/documents/sha256/${documentDigest.slice(7)}.md`;

const channels = {
  "linked-cargo": [
    "marketplace_linked_cargo",
    "lenso.marketplace.linked-cargo-snapshot.v1",
  ],
  packages: ["marketplace_packages", "lenso.marketplace.package-snapshot.v1"],
  "release-content": [
    "marketplace_release_content",
    "lenso.marketplace.release-content.v2",
  ],
  "release-details": [
    "marketplace_release_details",
    "lenso.marketplace.release-details.v1",
  ],
};

const fixture = (
  channel = "release-details",
  expiresAt = Math.floor(Date.now() / 1000) + 3600
) => {
  const [table, schema] = channels[channel];
  const envelopeBytes = new TextEncoder().encode(
    JSON.stringify({
      key_id: "test-key",
      payload_base64: Buffer.from(
        JSON.stringify({
          catalog_id: "catalog",
          expires_at: expiresAt,
          issued_at: Math.floor(Date.now() / 1000) - 60,
          releases: [
            channel === "release-content"
              ? {
                  metadata: {
                    documentation: [
                      {
                        digest: documentDigest,
                        media_type: "text/markdown",
                        size: body.byteLength,
                      },
                    ],
                  },
                }
              : {
                  documentation: [
                    {
                      digest: documentDigest,
                      media_type: "text/markdown",
                      size: body.byteLength,
                    },
                  ],
                },
          ],
          revision: 1,
          schema,
        })
      ).toString("base64"),
      signature_base64: "fixture-signature-verified-by-protected-publisher",
    })
  );
  const publicationDigest = `sha256:${createHash("sha256").update(envelopeBytes).digest("hex")}`;
  const pointer = {
    digest: publicationDigest,
    object_key: `${channel}/catalog/${publicationDigest.slice(7)}.json`,
    revision: 1,
  };
  const objects = new Map([
    [pointer.object_key, envelopeBytes],
    [`documents/sha256/${documentDigest.slice(7)}.md`, body],
  ]);
  const reads = [];
  const env = {
    CATALOG_ID: "catalog",
    MARKETPLACE_DB: {
      prepare(sql) {
        return {
          bind(catalog) {
            assert.equal(catalog, "catalog");
            return {
              first() {
                reads.push(sql);
                return sql.includes(table) ? pointer : null;
              },
            };
          },
        };
      },
    },
    MARKETPLACE_OBJECTS: {
      get(key) {
        reads.push(key);
        const bytes = objects.get(key);
        return bytes
          ? { body: new Blob([bytes]).stream(), size: bytes.byteLength }
          : null;
      },
    },
  };
  return { channel, env, objects, pointer, reads };
};

test("serves only a current signed-public document by exact digest", async () => {
  for (const channel of Object.keys(channels)) {
    const { env } = fixture(channel);
    const response = await documentResponse(
      new Request(`https://marketplace.test${path}`),
      env
    );
    assert.equal(response.status, 200, channel);
    assert.equal(
      response.headers.get("content-type"),
      "text/markdown; charset=utf-8"
    );
    assert.equal(
      response.headers.get("x-lenso-document-digest"),
      documentDigest
    );
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.deepEqual(new Uint8Array(await response.arrayBuffer()), body);
    const head = await documentResponse(
      new Request(`https://marketplace.test${path}`, { method: "HEAD" }),
      env
    );
    assert.equal(head.status, 200);
    assert.equal(head.headers.get("content-length"), String(body.byteLength));
    assert.equal(await head.text(), "");
  }
});

test("does not expose an R2 body absent from current public documentation", async () => {
  const { env, objects, pointer } = fixture();
  const other = `sha256:${"b".repeat(64)}`;
  objects.set(`documents/sha256/${other.slice(7)}.md`, body);
  const unknown = await documentResponse(
    new Request(
      `https://marketplace.test/documents/sha256/${other.slice(7)}.md`
    ),
    env
  );
  assert.equal(unknown.status, 404);
  pointer.digest = `sha256:${"c".repeat(64)}`;
  pointer.object_key = `release-details/catalog/${pointer.digest.slice(7)}.json`;
  const stale = await documentResponse(
    new Request(`https://marketplace.test${path}`),
    env
  );
  assert.equal(stale.status, 503);
});

test("retains a published exact revision after snapshot freshness expires", async () => {
  const { env } = fixture("release-details", 1);
  const response = await documentResponse(
    new Request(`https://marketplace.test${path}`),
    env
  );
  assert.equal(response.status, 200);
  assert.deepEqual(new Uint8Array(await response.arrayBuffer()), body);
});

test("fails closed on damaged content and unsupported methods", async () => {
  const { env, objects } = fixture();
  objects.set(
    `documents/sha256/${documentDigest.slice(7)}.md`,
    new TextEncoder().encode("corrupt")
  );
  const damaged = await documentResponse(
    new Request(`https://marketplace.test${path}`),
    env
  );
  assert.equal(damaged.status, 503);
  const post = await documentResponse(
    new Request(`https://marketplace.test${path}`, { method: "POST" }),
    env
  );
  assert.equal(post.status, 405);
  assert.equal(
    await documentResponse(new Request("https://marketplace.test/"), env),
    null
  );
});
