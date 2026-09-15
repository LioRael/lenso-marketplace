import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import { createStorage } from "./storage.mjs";

test("R2 body returned after cancellation is still released", async () => {
  const controller = new AbortController();
  let cancelled = false;
  const object = {
    body: new ReadableStream({
      cancel() {
        cancelled = true;
      },
    }),
    size: 2,
  };
  const database = {
    prepare() {
      return {
        bind() {
          return {
            first: () =>
              Promise.resolve({ digest: "unused", object_key: "snapshot" }),
          };
        },
      };
    },
  };
  const bucket = {
    get() {
      controller.abort();
      return Promise.resolve(object);
    },
    put() {
      throw new Error("unexpected write");
    },
  };
  const storage = createStorage(database, bucket, controller.signal);
  await assert.rejects(
    storage("published", JSON.stringify({ catalog: "fixture" })),
    /abort/iu
  );
  assert.equal(cancelled, true);
});

test("cancelled absent-object lookup cannot initiate a durable write", async () => {
  const controller = new AbortController();
  let writes = 0;
  const bucket = {
    get() {
      controller.abort();
      return Promise.resolve(null);
    },
    put() {
      writes += 1;
      return Promise.resolve();
    },
  };
  const database = {
    prepare() {
      throw new Error("unexpected database access");
    },
  };
  const envelope = "{}";
  const token = `sha256:${createHash("sha256").update(envelope).digest("hex")}`;
  const storage = createStorage(database, bucket, controller.signal);
  await assert.rejects(
    storage(
      "compare_exchange",
      JSON.stringify({
        catalog: "fixture",
        expected: null,
        value: { checkpoint: {}, envelope, token },
      })
    ),
    /abort/iu
  );
  assert.equal(writes, 0);
});
