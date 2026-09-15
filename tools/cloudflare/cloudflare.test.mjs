import assert from "node:assert/strict";
import test from "node:test";

import { boundedJson, cloudflareStorage } from "./cloudflare.mjs";

const config = {
  accessKeyId: "fixture-access",
  accountId: "a".repeat(32),
  bucketName: "fixture-bucket",
  databaseId: "11111111-1111-1111-1111-111111111111",
  secretAccessKey: "fixture-secret",
  token: "fixture-token",
};

test("R2 signs create-only writes without replaying transient failures", async () => {
  let calls = 0;
  const { bucket } = cloudflareStorage({
    ...config,
    transport(request, init) {
      calls += 1;
      assert.equal(init.redirect, "error");
      assert.equal(request.method, "PUT");
      assert.equal(request.headers.get("if-none-match"), "*");
      assert.match(request.headers.get("authorization"), /^AWS4-HMAC-SHA256 /u);
      assert.equal(
        new URL(request.url).pathname,
        "/fixture-bucket/publications/catalog%252Fone/item.json"
      );
      return Promise.resolve(new Response("unavailable", { status: 503 }));
    },
  });
  await assert.rejects(
    bucket.put(
      "publications/catalog%2Fone/item.json",
      new TextEncoder().encode("{}"),
      { onlyIf: { etagDoesNotMatch: "*" } }
    ),
    /unconfirmed/u
  );
  assert.equal(calls, 1);
});

test("D1 rejects API-level failures and never retries a mutation", async () => {
  let calls = 0;
  const { database } = cloudflareStorage({
    ...config,
    transport(url, init) {
      calls += 1;
      assert.equal(new URL(url).hostname, "api.cloudflare.com");
      assert.equal(init.redirect, "error");
      assert.deepEqual(JSON.parse(init.body).params, [1]);
      return Promise.resolve(
        Response.json({ result: [{ success: false }], success: true })
      );
    },
  });
  await assert.rejects(
    database.prepare("UPDATE fixture SET revision=?").bind(1).run(),
    /unconfirmed/u
  );
  assert.equal(calls, 1);
});

test("oversized operator responses cancel the stream", async () => {
  let cancelled = false;
  const response = new Response(
    new ReadableStream({
      cancel() {
        cancelled = true;
      },
      pull(controller) {
        controller.enqueue(new Uint8Array(32));
      },
    })
  );
  await assert.rejects(boundedJson(response, 16), /exceeds limit/u);
  assert.equal(cancelled, true);
});
