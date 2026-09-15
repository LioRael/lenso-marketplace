import assert from "node:assert/strict";
import test from "node:test";

import { promotePublication } from "./promote.mjs";

const storage = () => {
  const state = {
    afterPut: null,
    fault: null,
    objects: new Map(),
    pointer: null,
    puts: 0,
    writes: 0,
  };
  const bucket = {
    get: (key) => {
      const value = state.objects.get(key);
      return value
        ? { body: new Blob([value]).stream(), size: value.length }
        : null;
    },
    put: (key, value, options) => {
      assert.equal(options.onlyIf.etagDoesNotMatch, "*");
      state.puts += 1;
      if (state.objects.has(key)) {
        return null;
      }
      state.objects.set(key, Uint8Array.from(value));
      state.afterPut?.();
      return { key };
    },
  };
  const database = {
    prepare(sql) {
      return {
        bind(...args) {
          return {
            first: () => state.pointer && { ...state.pointer },
            run: () => {
              state.writes += 1;
              assert.match(sql, /marketplace_publications/u);
              assert.doesNotMatch(sql, /marketplace_accepted/u);
              if (state.fault === "before") {
                throw new Error("transport lost");
              }
              let changes = 0;
              if (sql.startsWith("INSERT") && state.pointer === null) {
                state.pointer = {
                  digest: args[3],
                  object_key: args[2],
                  revision: args[1],
                };
                changes = 1;
              } else if (
                sql.startsWith("UPDATE") &&
                state.pointer?.revision === args[4] &&
                state.pointer?.object_key === args[5] &&
                state.pointer?.digest === args[6]
              ) {
                state.pointer = {
                  digest: args[2],
                  object_key: args[1],
                  revision: args[0],
                };
                changes = 1;
              }
              if (state.fault === "after") {
                throw new Error("receipt lost");
              }
              return { meta: { changes }, success: true };
            },
          };
        },
      };
    },
  };
  const input = (revision, expected = null, extra = "") => ({
    bucket,
    catalogId: "catalog",
    database,
    envelope: JSON.stringify({
      catalog_id: "catalog",
      expires_at: 100,
      extra,
      revision,
    }),
    expected,
    now: () => 50,
    // Storage tests inject trusted verification. Real trust is checked by Rust.
    verify: (envelope) => JSON.parse(envelope),
  });
  return { input, state };
};

test("publishes immutable bytes, advances a full pointer and reconciles exact retry", async () => {
  const { state, input } = storage();
  const first = await promotePublication(input(1));
  assert.equal(first.status, "published");
  const retry = await promotePublication(input(1));
  assert.equal(retry.status, "already_published");
  assert.equal(state.writes, 1);
  const previous = { ...state.pointer };
  const next = await promotePublication(input(2, previous));
  assert.equal(next.status, "published");
  await assert.rejects(
    promotePublication(input(3, previous)),
    /pointer conflict/u
  );
  assert.equal(state.pointer.revision, 2);
});

test("concurrent different publications cannot both replace the same pointer", async () => {
  const { state, input } = storage();
  const outcomes = await Promise.allSettled([
    promotePublication(input(1, null, "a")),
    promotePublication(input(1, null, "b")),
  ]);
  assert.equal(outcomes.filter((v) => v.status === "fulfilled").length, 1);
  assert.equal(state.pointer.revision, 1);
});

test("lost D1 receipt is reconciled without replaying the write", async () => {
  const { state, input } = storage();
  state.fault = "after";
  const receipt = await promotePublication(input(1));
  assert.equal(receipt.status, "reconciled");
  assert.equal(state.writes, 1);
});

test("unconfirmed write stays uncertain and never retries", async () => {
  const { state, input } = storage();
  state.fault = "before";
  await assert.rejects(promotePublication(input(1)), /write unconfirmed/u);
  assert.equal(state.writes, 1);
  assert.equal(state.pointer, null);
});

test("expiry during object upload prevents pointer publication", async () => {
  const { state, input } = storage();
  let now = 50;
  state.afterPut = () => {
    now = 100;
  };
  await assert.rejects(
    promotePublication({ ...input(1), now: () => now }),
    /expired/u
  );
  assert.equal(state.writes, 0);
});

test("corrupted published bytes fail even for an idempotent retry", async () => {
  const { state, input } = storage();
  const receipt = await promotePublication(input(1));
  state.objects.set(receipt.object_key, new TextEncoder().encode("corrupt"));
  await assert.rejects(promotePublication(input(1)), /integrity/u);
  assert.equal(state.writes, 1);
});
