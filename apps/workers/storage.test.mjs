import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
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
              Promise.resolve({
                digest: `sha256:${"0".repeat(64)}`,
                object_key: "snapshot",
              }),
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

const hash = (value) =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;
const catalog = "conformance-only";
const input = JSON.stringify({ catalog });

// Keep the committed cross-language signature and payload bytes unchanged.
const vector = JSON.parse(
  await readFile(
    new URL("../../tests/support/tests/conformance.json", import.meta.url),
    "utf-8"
  )
);
const signedEnvelope = `${JSON.stringify(vector.envelope)}\n`;

const fixture = (envelope = signedEnvelope) => {
  const accepted = {
    checkpoint: { history: { retained: "identity" }, revision: 1 },
    envelope,
    token: hash(envelope),
  };
  const raw = JSON.stringify(accepted);
  const key = `accepted/${encodeURIComponent(catalog)}/${hash(raw).slice(7)}.json`;
  const state = {
    calls: { batch: 0, first: 0, get: [], put: 0, run: 0 },
    controller: new AbortController(),
    objects: new Map([
      [key, raw],
      ["proof/first.json", envelope],
    ]),
    pointer: { object_key: key, token: accepted.token },
    publication: { digest: hash(envelope), object_key: "proof/first.json" },
  };
  const database = {
    batch(statements) {
      state.calls.batch += 1;
      assert.deepEqual(
        statements.map((statement) => statement.table),
        ["publication", "pointer"]
      );
      return Promise.resolve(
        statements.map(({ table }) => ({
          results: state[table] ? [{ ...state[table] }] : [],
          success: true,
        }))
      );
    },
    prepare(sql) {
      return {
        bind(...args) {
          assert.deepEqual(args, [catalog]);
          const table = sql.includes("marketplace_publications")
            ? "publication"
            : "pointer";
          assert.equal(
            sql,
            table === "publication"
              ? "SELECT object_key,digest FROM marketplace_publications WHERE catalog_id=?"
              : "SELECT token,object_key FROM marketplace_accepted WHERE catalog_id=?"
          );
          return {
            first() {
              state.calls.first += 1;
              return Promise.resolve(state[table]);
            },
            table,
          };
        },
      };
    },
  };
  const bucket = {
    get(objectKey) {
      state.calls.get.push(objectKey);
      const value = state.objects.get(objectKey);
      return Promise.resolve(
        value === undefined
          ? null
          : {
              body: new Response(value).body,
              size: Buffer.byteLength(value),
            }
      );
    },
    put() {
      state.calls.put += 1;
      throw new Error("unexpected write");
    },
  };
  const storage = createStorage(database, bucket, state.controller.signal);
  return { accepted, bucket, database, key, state, storage };
};

test("unchanged signed bytes use exactly one D1 batch and one accepted R2 GET on every read", async () => {
  const { state, storage, accepted, key } = fixture();
  for (let index = 0; index < 3; index += 1) {
    const result = JSON.parse(await storage("catalog", input));
    assert.deepEqual(result, { accepted, envelope: signedEnvelope });
    assert.deepEqual(state.calls, {
      batch: index + 1,
      first: 0,
      get: Array.from({ length: index + 1 }, () => key),
      put: 0,
      run: 0,
    });
  }
});

test("different exact envelope bytes require one D1 batch and both R2 objects", async () => {
  const { state, storage, accepted, key } = fixture();
  // Same signed payload, different envelope bytes: token equality is exact.
  const changed = JSON.stringify(vector.envelope, null, 2);
  state.publication = {
    digest: hash(changed),
    object_key: "proof/changed.json",
  };
  state.objects.set("proof/changed.json", changed);
  assert.deepEqual(JSON.parse(await storage("catalog", input)), {
    accepted,
    envelope: changed,
  });
  assert.deepEqual(state.calls, {
    batch: 1,
    first: 0,
    get: [key, "proof/changed.json"],
    put: 0,
    run: 0,
  });
});

test("missing accepted state reads publication independently", async () => {
  const { state, storage } = fixture();
  state.pointer = null;
  assert.deepEqual(JSON.parse(await storage("catalog", input)), {
    accepted: null,
    envelope: signedEnvelope,
  });
  assert.deepEqual(state.calls, {
    batch: 1,
    first: 0,
    get: ["proof/first.json"],
    put: 0,
    run: 0,
  });
});

test("missing publication preserves prior state without presenting it as publication", async () => {
  const { state, storage, accepted, key } = fixture();
  state.publication = null;
  assert.deepEqual(JSON.parse(await storage("catalog", input)), {
    accepted,
    envelope: null,
  });
  assert.deepEqual(state.calls.get, [key]);
  state.pointer = null;
  assert.deepEqual(JSON.parse(await storage("catalog", input)), {
    accepted: null,
    envelope: null,
  });
  assert.equal(state.calls.batch, 2);
  assert.equal(state.calls.first, 0);
  assert.deepEqual(state.calls.get, [key]);
});

for (const kind of [
  "checkpoint",
  "object key",
  "pointer token",
  "envelope",
  "missing object",
  "invalid JSON",
  "non-string envelope",
]) {
  test(`coalesced read rejects corrupt accepted ${kind} before envelope reuse`, async () => {
    const { state, storage, accepted, key } = fixture();
    if (kind === "checkpoint") {
      accepted.checkpoint.history.retained = "tampered";
      state.objects.set(key, JSON.stringify(accepted));
    } else if (kind === "object key") {
      state.pointer.object_key = "accepted/other-catalog/alias.json";
      state.objects.set(state.pointer.object_key, state.objects.get(key));
    } else if (kind === "pointer token") {
      state.pointer.token = hash("other bytes");
    } else if (kind === "missing object") {
      state.objects.delete(key);
    } else {
      accepted.envelope = kind === "non-string envelope" ? {} : "tampered";
      const raw = kind === "invalid JSON" ? "{" : JSON.stringify(accepted);
      state.pointer.object_key = `accepted/${catalog}/${hash(raw).slice(7)}.json`;
      state.objects.set(state.pointer.object_key, raw);
    }
    await assert.rejects(storage("catalog", input));
    assert.equal(state.calls.batch, 1);
    assert.equal(state.calls.get.length, 1);
    assert.equal(state.calls.put, 0);
  });
}

for (const kind of [
  "digest",
  "missing object",
  "invalid UTF-8",
  "oversized object",
]) {
  test(`changed publication rejects ${kind}`, async () => {
    const { state, storage, key } = fixture();
    state.publication = {
      digest: hash("changed"),
      object_key: "proof/changed.json",
    };
    if (kind === "digest") {
      state.objects.set("proof/changed.json", signedEnvelope);
    } else if (kind === "invalid UTF-8") {
      state.objects.set("proof/changed.json", new Uint8Array([255]));
    } else if (kind === "oversized object") {
      state.objects.set("proof/changed.json", "x".repeat(4 * 1024 * 1024 + 1));
    }
    await assert.rejects(storage("catalog", input));
    assert.deepEqual(state.calls.get, [key, "proof/changed.json"]);
    assert.equal(state.calls.put, 0);
  });
}

test("accepted reads after a coalesced read fetch the durable winner anew", async () => {
  const { state, storage, accepted, key } = fixture();
  await storage("catalog", input);
  const winner = {
    ...accepted,
    checkpoint: { history: { retained: "winner" }, revision: 2 },
  };
  const raw = JSON.stringify(winner);
  const winnerKey = `accepted/${catalog}/${hash(raw).slice(7)}.json`;
  state.pointer = { object_key: winnerKey, token: winner.token };
  state.objects.set(winnerKey, raw);
  assert.deepEqual(JSON.parse(await storage("accepted", input)), winner);
  assert.deepEqual(state.calls, {
    batch: 1,
    first: 1,
    get: [key, winnerKey],
    put: 0,
    run: 0,
  });
});

test("raw publication reads remain independent of accepted corruption", async () => {
  const { state, storage, key } = fixture();
  state.objects.set(key, "corrupt");
  assert.equal(JSON.parse(await storage("published", input)), signedEnvelope);
  assert.deepEqual(state.calls, {
    batch: 0,
    first: 1,
    get: ["proof/first.json"],
    put: 0,
    run: 0,
  });
});

test("failed or malformed D1 batch cannot become missing accepted state", async () => {
  for (const rows of [
    [],
    [{ success: false }, { results: [], success: true }],
    [{ success: true }, { results: [], success: true }],
  ]) {
    const { state, database, bucket } = fixture();
    database.batch = () => Promise.resolve(rows);
    const storage = createStorage(database, bucket, state.controller.signal);
    await assert.rejects(
      storage("catalog", input),
      /catalog pointer read failed/u
    );
    assert.deepEqual(state.calls.get, []);
  }
});

test("cancellation after the pointer batch prevents R2 reads", async () => {
  const { state, database, bucket } = fixture();
  const { batch } = database;
  database.batch = (statements) => {
    state.controller.abort();
    return batch(statements);
  };
  const storage = createStorage(database, bucket, state.controller.signal);
  await assert.rejects(storage("catalog", input), /abort/iu);
  assert.deepEqual(state.calls.get, []);
});

for (const pointer of [
  { digest: hash(signedEnvelope), object_key: null },
  { digest: hash(signedEnvelope), object_key: "" },
  { digest: "sha256:invalid", object_key: "proof/first.json" },
]) {
  test(`invalid publication pointer cannot authorize reuse: ${JSON.stringify(pointer)}`, async () => {
    const { state, storage } = fixture();
    state.publication = pointer;
    await assert.rejects(
      storage("catalog", input),
      /invalid publication pointer/u
    );
    assert.deepEqual(state.calls.get, []);
  });
}
