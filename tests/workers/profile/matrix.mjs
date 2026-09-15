import assert from "node:assert/strict";
import { once } from "node:events";
import { readFile } from "node:fs/promises";
import { createServer } from "node:net";
import { join } from "node:path";
import { performance } from "node:perf_hooks";

import { Miniflare } from "miniflare";

import { catalogId, fixtures, keyId, sha256 } from "./fixtures.mjs";

const route = "/api/marketplace/v1/plugins?limit=1";
const rawRoute = "/api/marketplace/v1/snapshot";
const samples = 32;
const coldSamples = 3;

export const distribution = (values) => {
  if (values.length === 0) {
    return { p50: null, p95: null, p99: null, samples: 0 };
  }
  const sorted = values.toSorted((a, b) => a - b);
  const percentile = (p) => sorted[Math.ceil(p * sorted.length) - 1];
  return {
    p50: percentile(0.5),
    p95: percentile(0.95),
    p99: percentile(0.99),
    samples: sorted.length,
  };
};

const sum = (items, field) =>
  items.reduce((total, item) => total + (item[field] ?? 0), 0);

export const counters = (trace) => {
  const batches = trace.filter((event) => event.kind === "d1.batch");
  const writes = trace.filter((event) => event.kind === "d1.run");
  const reads = trace.filter((event) => event.kind === "d1.first");
  const gets = trace.filter((event) => event.kind === "r2.get");
  const puts = trace.filter((event) => event.kind === "r2.put");
  const batchMeta = batches.flatMap((event) => event.meta);
  return {
    d1BatchRowsRead: sum(batchMeta, "rows_read"),
    d1BatchRowsReturned: sum(batches, "rowsReturned"),
    d1BatchRowsWritten: sum(batchMeta, "rows_written"),
    d1BatchStatements: sum(batches, "statements"),
    d1Batches: batches.length,
    d1FirstCalls: reads.length,
    d1FirstRowsReturned: sum(reads, "rowsReturned"),
    d1RunCalls: writes.length,
    d1RunRowsRead: sum(
      writes.map((event) => event.meta),
      "rows_read"
    ),
    d1RunRowsWritten: sum(
      writes.map((event) => event.meta),
      "rows_written"
    ),
    r2GetBytes: sum(gets, "bytes"),
    r2GetCalls: gets.length,
    r2PutBytes: sum(puts, "bytes"),
    r2PutCalls: puts.length,
  };
};

const request = async (runtime, pathname = route, headers = {}) => {
  const start = performance.now();
  // Node's fetch traverses the loopback HTTP listener and drains the full body.
  const response = await fetch(new URL(pathname, runtime.url), {
    headers,
    signal: AbortSignal.timeout(15_000),
  });
  const bytes = Buffer.from(await response.arrayBuffer());
  const externalLatencyMs = performance.now() - start;
  const receipt = JSON.parse(response.headers.get("x-d01-receipt") ?? "null");
  const trace = JSON.parse(response.headers.get("x-d01-storage") ?? "[]");
  const record = {
    externalLatencyMs,
    receipt,
    responseBytes: bytes.length,
    responseSha256: sha256(bytes),
    status: response.status,
    totals: counters(trace),
    trace,
  };
  return { bytes, record };
};

const assertBrowse = (result, target) => {
  assert.equal(result.record.status, 200);
  assert.equal(result.record.receipt?.shutdown, "clean");
  assert.ok(result.record.receipt.wasmMemoryBytes > 0);
  const body = JSON.parse(result.bytes);
  assert.equal(body.catalog_id, catalogId);
  assert.equal(body.revision, target.revision);
  assert.equal(body.total, target.snapshot.releases.length);
  assert.deepEqual(body.releases, [target.snapshot.releases[0]]);
  assert.equal(body.cached, false);
  assert.equal(body.stale, true);
};

const publish = async (runtime, entry) => {
  await runtime.bucket.put(entry.object_key, entry.envelope);
  await runtime.database
    .prepare("INSERT OR REPLACE INTO marketplace_publications VALUES(?,?,?,?)")
    .bind(catalogId, entry.revision, entry.object_key, entry.digest)
    .run();
};

const accepted = async (runtime) => {
  const pointer = await runtime.database
    .prepare("SELECT * FROM marketplace_accepted WHERE catalog_id=?")
    .bind(catalogId)
    .first();
  const object = await runtime.bucket.get(pointer.object_key);
  const raw = await object.text();
  assert.equal(pointer.object_key, `accepted/${catalogId}/${sha256(raw)}.json`);
  const state = JSON.parse(raw);
  assert.equal(pointer.token, `sha256:${sha256(state.envelope)}`);
  return { pointer, raw, state };
};

const restore = async (runtime, stored) => {
  await runtime.bucket.put(stored.pointer.object_key, stored.raw);
  await runtime.database
    .prepare("INSERT OR REPLACE INTO marketplace_accepted VALUES(?,?,?)")
    .bind(catalogId, stored.pointer.token, stored.pointer.object_key)
    .run();
};

export const summarizeMatrix = (evidence) => {
  for (const cell of evidence.cells ?? []) {
    for (const boundary of [
      "cold-process-fresh-app",
      "warm-isolate-warm-generation-fresh-app",
      "warm-isolate-new-generation-fresh-app",
    ]) {
      const records = cell.records.filter(
        (record) => record.boundary === boundary
      );
      cell.summaries[boundary] = {
        artifactProcessStartupMs: distribution(
          records.flatMap((record) =>
            record.artifactProcessStartupMs === null
              ? []
              : [record.artifactProcessStartupMs]
          )
        ),
        catalogRead: Object.fromEntries(
          Object.keys(counters([])).map((key) => [
            key,
            distribution(
              records.flatMap((record) =>
                record.catalogRead ? [record.catalogRead[key]] : []
              )
            ),
          ])
        ),
        externalLatencyMs: distribution(
          records.map((record) => record.externalLatencyMs)
        ),
        failureCount: records.filter((record) => !record.assertionsPassed)
          .length,
        responseBytes: distribution(
          records.map((record) => record.responseBytes)
        ),
        totals: Object.fromEntries(
          Object.keys(counters([])).map((key) => [
            key,
            distribution(records.map((record) => record.totals[key])),
          ])
        ),
        wasmLinearMemoryBytes: distribution(
          records.flatMap((record) =>
            record.receipt ? [record.receipt.wasmMemoryBytes] : []
          )
        ),
      };
    }
  }
};

export const runMatrix = async ({ artifact, temporary, evidence }) => {
  const { entries, publicKeyHex } = fixtures();
  let ordinal = 0;
  evidence.fixtureIdentities = Object.fromEntries(
    Object.entries(entries).map(([name, entry]) => [
      name,
      { bytes: entry.bytes, revision: entry.revision, sha256: entry.sha256 },
    ])
  );
  evidence.samplesPerWarmCell = samples;
  evidence.samplesPerColdCell = coldSamples;
  evidence.operation = route;
  evidence.cells = ["small", "large"].flatMap((size) =>
    ["unchanged", "changed"].map((path) => ({
      path,
      records: [],
      size,
      summaries: {},
    }))
  );
  evidence.assertions = [];
  // Miniflare's proxy listener may emit an unhandled error under a sandbox.
  // Fail as an explicit prerequisite before it can discard the machine receipt.
  const server = createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const closed = once(server, "close");
  server.close();
  await closed;
  const migrationText = await readFile(
    new URL(
      "../../../apps/workers/migrations/d1/0001_public_reads.sql",
      import.meta.url
    ),
    "utf-8"
  );
  const migration = migrationText
    .replaceAll(/--[^\n]*/gu, "")
    .replaceAll("\n", " ");

  const withRuntime = async (label, action) => {
    ordinal += 1;
    const instance = ordinal;
    const started = performance.now();
    const runtime = new Miniflare({
      bindings: {
        CATALOG_ID: catalogId,
        CATALOG_KEY_ID: keyId,
        CATALOG_PUBLIC_KEY: publicKeyHex,
      },
      compatibilityDate: "2026-07-08",
      compatibilityFlags: [
        "global_fetch_strictly_public",
        "enable_request_signal",
      ],
      d1Databases: ["MARKETPLACE_DB"],
      d1Persist: join(temporary, String(instance), "d1"),
      host: "127.0.0.1",
      modules: true,
      modulesRoot: artifact,
      modulesRules: [{ include: ["**/*.wasm"], type: "CompiledWasm" }],
      // A catalog must never cause an outbound request, even to fixture URLs.
      outboundService: () =>
        new Response("Outbound access forbidden", { status: 502 }),
      port: 0,
      r2Buckets: ["MARKETPLACE_OBJECTS"],
      r2Persist: join(temporary, String(instance), "r2"),
      scriptPath: join(artifact, "worker.js"),
    });
    try {
      const url = await runtime.ready;
      const artifactProcessStartupMs = performance.now() - started;
      const database = await runtime.getD1Database("MARKETPLACE_DB");
      const bucket = await runtime.getR2Bucket("MARKETPLACE_OBJECTS");
      await database.exec(migration);
      await action({
        artifactProcessStartupMs,
        bucket,
        database,
        instance,
        label,
        url,
      });
    } finally {
      await runtime.dispose();
    }
  };

  // Only the real Rust/Wasm consumer constructs these checkpoint objects.
  const states = {};
  await withRuntime("bootstrap", async (runtime) => {
    for (const size of ["small", "large"]) {
      for (const name of ["prior", "target", "newer", "equivocation"]) {
        const key = `${size}-${name}`;
        const entry = entries[key];
        await runtime.database
          .prepare("DELETE FROM marketplace_accepted")
          .run();
        await publish(runtime, entry);
        const result = await request(runtime);
        assertBrowse(result, entry);
        states[key] = await accepted(runtime);
        assert.equal(states[key].state.envelope, entry.envelope);
        const raw = await request(runtime, rawRoute);
        assert.equal(raw.record.status, 200);
        assert.equal(raw.bytes.toString(), entry.envelope);
        evidence.assertions.push({
          exactEnvelopeSha256: sha256(raw.bytes),
          name: `bootstrap-${key}`,
          passed: true,
          receipt: result.record.receipt,
        });
      }
    }
  });

  for (const size of ["small", "large"]) {
    for (const path of ["unchanged", "changed"]) {
      const target = entries[`${size}-target`];
      const prior =
        states[`${size}-${path === "unchanged" ? "target" : "prior"}`];
      const expected = states[`${size}-target`];
      const cell = evidence.cells.find(
        (entry) => entry.size === size && entry.path === path
      );
      for (let cold = 0; cold < coldSamples; cold += 1) {
        await withRuntime(`${size}-${path}-${cold}`, async (runtime) => {
          let generation;
          let admissions = 0;
          for (
            let index = 0;
            index < (cold === 0 ? samples + 1 : 1);
            index += 1
          ) {
            // Fixture mutations are excluded from request latency and counters.
            await publish(runtime, target);
            await restore(runtime, prior);
            if (path === "changed") {
              await runtime.bucket.delete(expected.pointer.object_key);
            }
            const result = await request(runtime);
            const { record } = result;
            const nextGeneration = record.receipt?.generation;
            let boundary = "warm-isolate-warm-generation-fresh-app";
            if (index === 0) {
              boundary = "cold-process-fresh-app";
            } else if (nextGeneration !== generation) {
              boundary = "warm-isolate-new-generation-fresh-app";
            }
            Object.assign(record, {
              artifactProcessStartupMs:
                index === 0 ? runtime.artifactProcessStartupMs : null,
              boundary,
              instance: runtime.instance,
            });
            cell.records.push(record);
            assertBrowse(result, target);
            if (generation !== nextGeneration) {
              if (generation !== undefined) {
                assert.equal(
                  admissions,
                  16,
                  "retirement stays at 16 admissions"
                );
                assert.equal(nextGeneration, generation + 1);
              }
              admissions = 0;
            }
            generation = nextGeneration;
            admissions += 1;
            assert.ok(admissions <= 16);
            const getCount = path === "unchanged" ? 1 : 2;
            const initial = record.trace.slice(0, 1 + getCount);
            assert.deepEqual(
              initial.map((event) => event.kind),
              ["d1.batch", ...Array.from({ length: getCount }, () => "r2.get")]
            );
            assert.equal(initial[1].key, prior.pointer.object_key);
            if (path === "changed") {
              assert.equal(initial[2].key, target.object_key);
            }
            record.catalogRead = counters(initial);
            assert.equal(record.catalogRead.d1Batches, 1);
            assert.equal(record.catalogRead.d1BatchStatements, 2);
            assert.equal(record.catalogRead.d1BatchRowsReturned, 2);
            assert.equal(record.totals.d1Batches, 1);
            assert.equal(record.totals.d1FirstCalls, 0);
            assert.equal(
              record.totals.r2GetCalls,
              path === "unchanged" ? 1 : 3
            );
            assert.equal(
              record.totals.d1RunCalls,
              path === "unchanged" ? 0 : 1
            );
            assert.equal(
              record.totals.r2PutCalls,
              path === "unchanged" ? 0 : 1
            );
            if (path === "changed") {
              const write = record.trace.find(
                (event) => event.kind === "d1.run"
              );
              assert.equal(write.meta.changes, 1);
              assert.equal(write.args.at(-1), prior.pointer.token);
            }
            const stored = await accepted(runtime);
            assert.deepEqual(
              stored,
              expected,
              "exact Rust checkpoint and signed bytes"
            );
            record.acceptedToken = stored.pointer.token;
            record.acceptedStateSha256 = sha256(stored.raw);
            record.assertionsPassed = true;
          }
        });
      }
    }
  }

  // Exercise failures through the same real App and bindings, outside the matrix.
  await withRuntime("trust-cas", async (runtime) => {
    for (const size of ["small", "large"]) {
      const target = entries[`${size}-target`];
      const prior = states[`${size}-prior`];
      for (const [label, publication, before, winner, status] of [
        [
          "rollback",
          entries[`${size}-prior`],
          states[`${size}-target`],
          null,
          503,
        ],
        [
          "equivocation",
          entries[`${size}-equivocation`],
          states[`${size}-target`],
          null,
          503,
        ],
        ["cas-newer-winner", target, prior, states[`${size}-newer`], 503],
        [
          "cas-equivocation-winner",
          target,
          prior,
          states[`${size}-equivocation`],
          503,
        ],
        ["cas-identical-winner", target, prior, states[`${size}-target`], 200],
      ]) {
        await publish(runtime, publication);
        await restore(runtime, before);
        if (winner) {
          await runtime.bucket.put(winner.pointer.object_key, winner.raw);
        }
        const result = await request(
          runtime,
          route,
          winner
            ? {
                "x-d01-winner": JSON.stringify(winner.pointer),
              }
            : {}
        );
        assert.equal(result.record.status, status, label);
        assert.deepEqual(await accepted(runtime), winner ?? before);
        if (winner) {
          assert.equal(result.record.totals.d1FirstCalls, 1);
          const write = result.record.trace.find(
            (event) => event.kind === "d1.run"
          );
          assert.equal(write.meta.changes, 0);
          const reread = result.record.trace.findLast(
            (event) => event.kind === "r2.get"
          );
          assert.equal(reread.key, winner.pointer.object_key);
        }
        evidence.assertions.push({
          name: `${size}-${label}`,
          passed: true,
          ...result.record,
        });
      }
      for (const kind of [
        "signature",
        "checkpoint",
        "object-integrity",
        "trust",
      ]) {
        const original = states[`${size}-target`];
        const state = structuredClone(original.state);
        let publication = target;
        if (kind === "signature" || kind === "trust") {
          const envelope = JSON.parse(state.envelope);
          if (kind === "signature") {
            envelope.signature_base64 = `${"A".repeat(86)}==`;
          } else {
            envelope.key_id = "unconfigured-key";
          }
          state.envelope = JSON.stringify(envelope);
          state.token = `sha256:${sha256(state.envelope)}`;
          publication = {
            ...target,
            digest: state.token,
            envelope: state.envelope,
          };
        } else {
          state.checkpoint.payload_digest = `sha256:${"0".repeat(64)}`;
        }
        const raw = JSON.stringify(state);
        const corrupt = {
          pointer: {
            catalog_id: catalogId,
            object_key:
              kind === "object-integrity"
                ? original.pointer.object_key
                : `accepted/${catalogId}/${sha256(raw)}.json`,
            token: state.token,
          },
          raw,
          state,
        };
        await publish(runtime, publication);
        await restore(runtime, corrupt);
        const result = await request(runtime);
        assert.equal(result.record.status, 503, kind);
        assert.equal(result.record.totals.d1RunCalls, 0);
        const object = await runtime.bucket.get(corrupt.pointer.object_key);
        assert.equal(await object.text(), corrupt.raw);
        evidence.assertions.push({
          name: `${size}-${kind}`,
          passed: true,
          ...result.record,
        });
      }
    }
  });
};
