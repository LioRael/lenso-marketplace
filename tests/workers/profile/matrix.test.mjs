import assert from "node:assert/strict";
import test from "node:test";

import { fixtures } from "./fixtures.mjs";
import { counters, distribution } from "./matrix.mjs";

test("pinned public fixtures preserve signatures, exact bytes and equivalent release contents", () => {
  const { entries } = fixtures();
  for (const size of ["small", "large"]) {
    const prior = entries[`${size}-prior`];
    const target = entries[`${size}-target`];
    assert.notEqual(prior.digest, target.digest);
    assert.deepEqual(prior.snapshot.releases, target.snapshot.releases);
    assert.equal(prior.bytes, target.bytes);
  }
  assert.ok(entries["small-target"].bytes < 2048);
  assert.ok(entries["large-target"].bytes > 3 * 1024 * 1024);
  assert.ok(entries["large-target"].bytes < 4 * 1024 * 1024);
  assert.deepEqual(
    entries["small-target"].snapshot.releases[0],
    entries["large-target"].snapshot.releases[0]
  );
});

test("empty evidence has no invented percentiles; nearest rank includes tails", () => {
  assert.deepEqual(distribution([]), {
    p50: null,
    p95: null,
    p99: null,
    samples: 0,
  });
  assert.deepEqual(distribution([100, 1, 2, 3, 4]), {
    p50: 3,
    p95: 100,
    p99: 100,
    samples: 5,
  });
});

test("R2 missing-object GETs and acceptance writes remain in full event counters", () => {
  const totals = counters([
    {
      kind: "d1.batch",
      meta: [
        { rows_read: 1, rows_written: 0 },
        { rows_read: 1, rows_written: 0 },
      ],
      rowsReturned: 2,
      statements: 2,
    },
    { bytes: 1000, kind: "r2.get" },
    { bytes: 900, kind: "r2.get" },
    { bytes: 0, kind: "r2.get" },
    { bytes: 1200, kind: "r2.put" },
    { kind: "d1.run", meta: { rows_read: 1, rows_written: 1 } },
    { kind: "fixture.race", meta: { rows_written: 1 } },
  ]);
  assert.equal(totals.d1Batches, 1);
  assert.equal(totals.d1BatchRowsRead, 2);
  assert.equal(totals.r2GetCalls, 3);
  assert.equal(totals.r2GetBytes, 1900);
  assert.equal(totals.d1RunRowsWritten, 1);
  assert.equal(totals.r2PutBytes, 1200);
});
