import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import test from "node:test";

import { keylessResponse } from "./keyless-current.mjs";

globalThis.crypto ??= webcrypto;
const origin = "https://marketplace.lenso.dev";
const catalogId = "lenso-official-v2";
const row = {
  bundle_digest: "b".repeat(64),
  bundle_size: 200,
  catalog_digest: "a".repeat(64),
  catalog_id: catalogId,
  catalog_size: 100,
  revision: 1,
  source_sha: "c".repeat(40),
};
const environment = (head = row, object = null) => ({
  CATALOG_ID: catalogId,
  MARKETPLACE_DB: {
    withSession(mode) {
      assert.equal(mode, "first-primary");
      return {
        prepare(sql) {
          assert.match(
            sql,
            /FROM marketplace_keyless_heads WHERE catalog_id=\?/u
          );
          return {
            bind(id) {
              assert.equal(id, catalogId);
              return {
                first() {
                  return Promise.resolve(head);
                },
              };
            },
          };
        },
      };
    },
  },
  MARKETPLACE_OBJECTS: {
    get(key) {
      assert.match(key, /^keyless\/[a-f0-9]{64}\.json$/u);
      return Promise.resolve(object);
    },
  },
});
const request = (path, options) => new Request(`${origin}${path}`, options);
const currentPath = "/api/marketplace/v3/current";
const responseStatus = async (pending) => {
  const response = await pending;
  return response.status;
};

test("does not intercept legacy catalog requests", async () => {
  assert.equal(
    await keylessResponse(request("/api/marketplace/v1/snapshot"), {}),
    null
  );
});
test("returns a single primary-read head with fixed provenance policy", async () => {
  const response = await keylessResponse(request(currentPath), environment());
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("cache-control"), "no-store");
  const result = await response.json();
  assert.equal(result.schema, "lenso.marketplace.keyless-current.v1");
  assert.equal(result.catalog.sha256, row.catalog_digest);
  assert.equal(result.bundle.sha256, row.bundle_digest);
  assert.equal(result.provenance.repository, "LioRael/lenso-marketplace");
  assert.equal(result.provenance.ref, "refs/heads/main");
});
test("unpromoted keyless catalogs are unavailable, not unsigned fallback", async () => {
  assert.equal(
    await responseStatus(
      keylessResponse(request(currentPath), environment(null))
    ),
    503
  );
});
test("wrong-catalog pointers fail closed", async () => {
  assert.equal(
    await responseStatus(
      keylessResponse(
        request(currentPath),
        environment({ ...row, catalog_id: "other" })
      )
    ),
    503
  );
});
test("unsafe revisions fail closed", async () => {
  assert.equal(
    await responseStatus(
      keylessResponse(
        request(currentPath),
        environment({ ...row, revision: Number.MAX_SAFE_INTEGER + 1 })
      )
    ),
    503
  );
});
test("invalid object bounds fail closed", async () => {
  assert.equal(
    await responseStatus(
      keylessResponse(
        request(currentPath),
        environment({ ...row, bundle_size: 4194305 })
      )
    ),
    503
  );
});
test("mutations and query overrides are not accepted", async () => {
  assert.equal(
    await responseStatus(
      keylessResponse(request(currentPath, { method: "POST" }), {})
    ),
    405
  );
  assert.equal(
    await responseStatus(
      keylessResponse(request(`${currentPath}?catalog=other`), {})
    ),
    400
  );
});
test("HEAD reports availability without returning a pointer body", async () => {
  const response = await keylessResponse(
    request(currentPath, { method: "HEAD" }),
    environment()
  );
  assert.equal(response.status, 200);
  assert.equal(await response.text(), "");
});
test("immutable objects are byte-verified before serving", async () => {
  const bytes = new TextEncoder().encode('{"example":"public"}');
  const sha = Buffer.from(
    await crypto.subtle.digest("SHA-256", bytes)
  ).toString("hex");
  const object = { body: new Response(bytes).body, size: bytes.length };
  const response = await keylessResponse(
    request(`/api/marketplace/v3/objects/${sha}.json`),
    environment(row, object)
  );
  assert.equal(response.status, 200);
  assert.match(response.headers.get("cache-control"), /immutable/u);
  assert.equal(await response.text(), new TextDecoder().decode(bytes));
});
test("wrong immutable bytes cannot be served under a requested digest", async () => {
  const bytes = new TextEncoder().encode("tampered");
  const object = { body: new Response(bytes).body, size: bytes.length };
  assert.equal(
    await responseStatus(
      keylessResponse(
        request(`/api/marketplace/v3/objects/${"a".repeat(64)}.json`),
        environment(row, object)
      )
    ),
    503
  );
});
test("streamed length must match the declared bounded size", async () => {
  const object = { body: new Response("too long").body, size: 1 };
  assert.equal(
    await responseStatus(
      keylessResponse(
        request(`/api/marketplace/v3/objects/${"a".repeat(64)}.json`),
        environment(row, object)
      )
    ),
    503
  );
});
test("aborted binding queries return unavailability without waiting for a row", async () => {
  const env = environment();
  env.MARKETPLACE_DB.withSession = () => ({
    prepare: () => ({
      bind: () => ({ first: () => Promise.withResolvers().promise }),
    }),
  });
  const controller = new AbortController();
  const pending = keylessResponse(
    request(currentPath, { signal: controller.signal }),
    env
  );
  controller.abort();
  assert.equal(await responseStatus(pending), 503);
});
