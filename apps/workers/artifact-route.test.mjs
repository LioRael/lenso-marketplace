import assert from "node:assert/strict";
import test from "node:test";

import { artifactResponse } from "./artifacts.mjs";

const digest = "a".repeat(64);
const body = new TextEncoder().encode("bundle");

const environment = () => {
  const calls = [];
  return {
    MARKETPLACE_OBJECTS: {
      get(key) {
        calls.push(key);
        return { body: new Response(body).body, size: body.byteLength };
      },
    },
    calls,
  };
};

test("artifact route returns an immutable direct response", async () => {
  const env = environment();
  const response = await artifactResponse(
    new Request(`https://marketplace.test/artifacts/${digest}.lenso-plugin`),
    env
  );
  assert.equal(response.status, 200);
  assert.equal(
    response.headers.get("cache-control"),
    "public, max-age=31536000, immutable"
  );
  assert.equal(
    response.headers.get("content-type"),
    "application/vnd.lenso.plugin"
  );
  assert.equal(response.headers.get("content-length"), String(body.byteLength));
  assert.deepEqual(new Uint8Array(await response.arrayBuffer()), body);
  assert.deepEqual(env.calls, [`artifacts/${digest}.lenso-plugin`]);
});

test("artifact route does not intercept unrelated paths", async () => {
  const env = environment();
  assert.equal(
    await artifactResponse(new Request("https://marketplace.test/"), env),
    null
  );
  assert.deepEqual(env.calls, []);
});

test("artifact route rejects unsupported methods", async () => {
  const env = environment();
  const response = await artifactResponse(
    new Request(`https://marketplace.test/artifacts/${digest}.lenso-plugin`, {
      method: "POST",
    }),
    env
  );
  assert.equal(response.status, 405);
  assert.equal(response.headers.get("allow"), "GET, HEAD");
  assert.deepEqual(env.calls, []);
});

test("artifact route returns not found for an unknown digest", async () => {
  const env = {
    MARKETPLACE_OBJECTS: { get: () => null },
  };
  const response = await artifactResponse(
    new Request(
      `https://marketplace.test/artifacts/${"b".repeat(64)}.lenso-plugin`
    ),
    env
  );
  assert.equal(response.status, 404);
});
