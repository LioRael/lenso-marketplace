/* eslint-disable promise/avoid-new, promise/prefer-await-to-then -- Tests control native Promise settlement and observe whether an abandoned continuation runs; awaiting would deadlock the test. */
import assert from "node:assert/strict";
import test from "node:test";

import { createStorageScope } from "./storage-scope.mjs";

for (const kind of ["resolve", "reject"]) {
  test(`invalidated event fences late native ${kind}`, async () => {
    let complete;
    const scope = createStorageScope(
      () => () =>
        new Promise((resolve, reject) => {
          complete = kind === "resolve" ? resolve : reject;
        }),
      "{}"
    );
    let forwarded = false;
    void scope.storage("accepted", "{}").then(
      () => {
        forwarded = true;
      },
      () => {
        forwarded = true;
      }
    );
    scope.invalidate();
    complete(kind === "resolve" ? "null" : new Error("native failure"));
    assert.equal(await scope.settled(), true);
    assert.equal(forwarded, false);
  });
}

test("unabortable storage has bounded, stable uncertain settlement", async () => {
  let signal;
  const scope = createStorageScope((value) => {
    signal = value;
    return () =>
      new Promise(() => {
        /* Simulates a native operation that cannot settle or abort. */
      });
  }, "{}");
  void scope.storage("compare_exchange", "{}");
  scope.abort();
  assert.equal(signal.aborted, true);
  const settlement = scope.settled();
  assert.equal(scope.settled(), settlement);
  assert.equal(await settlement, false);
  scope.invalidate();
});

test("live storage results preserve the normal invocation", async () => {
  const scope = createStorageScope(() => () => Promise.resolve("null"), "{}");
  assert.equal(await scope.storage("accepted", "{}"), "null");
  scope.abort();
  assert.equal(await scope.settled(), true);
  scope.invalidate();
});
