# D01 local Marketplace Workers characterization

## Run

From the standalone Marketplace checkout:

```sh
node tools/workers/profile-matrix.mjs
```

Prerequisites: Node >=22.12, the repository's pnpm 11.5.0, Rust 1.94.0 with
Clippy, rustfmt and `wasm32-unknown-unknown`, wasm-bindgen CLI 0.2.127, and permission
to start local processes and listen on loopback. The command installs the frozen
pnpm lock, builds the UI before Rust checks, runs the Workers and Web Plugin tests,
wasm32 Clippy, Rust formatting, release build, production-entry Wrangler dry-run,
fixture tests, lint, formatting and diff checks. It then bundles the local test
entry with another Wrangler **dry-run** and runs the matrix. Dependencies remain
pinned by the existing locks; no dependency or required delivery check changes.

In the Lenso sibling workspace the command selects `.lenso-tools/bin/lenso-cargo`.
Elsewhere it uses `CARGO`, or `cargo`. A sandbox without access to the shared target
can set the wrapper's `LENSO_CARGO_CACHE_ROOT` to a disposable writable directory.
That caller-supplied cache remains the caller's responsibility to remove.

The default output is a new timestamped file under `docs/archive/workers-d01`.
An optional output filename is accepted; an existing file is never overwritten.
The command exits nonzero on a failed check, runtime error or assertion.
It still writes machine-readable failure evidence. Its `finally` paths dispose
Miniflare/workerd and remove task-owned D1/R2 databases, bundled artifacts and
logs. Build/dependency directories absent at entry are also removed; pre-existing
build caches are retained. Do not run another build in the same checkout during
the command.

## Equivalent cases

Every timed request is `GET /api/marketplace/v1/plugins?limit=1`. The first release
is identical at both sizes, keeping the returned content comparable while the
entire signed catalog and checkpoint pass through the real Rust consumer.
The small catalog has one release (1,045 envelope bytes); the large catalog has
161 releases (3,611,713 bytes), with 160 descriptions of 16 KiB. The fixture module reconstructs exact UTF-8 envelope
bytes from a fixed recipe and committed public signatures. It checks each length,
SHA-256 digest and Ed25519 signature before starting workerd. No private key is
stored or used by the command.

For each size, the unchanged and changed cases publish the **same revision-2
envelope**. Unchanged starts with its accepted state; changed starts with revision
1 containing exactly the same releases. Setup restores those local fixture
pointers before every sample, outside the timer and operation counters. A
separate bootstrap App obtains all checkpoint objects from Rust and checks the
raw snapshot route byte for byte. The harness never synthesizes a valid checkpoint
or substitutes a verifier. The fixed timestamps intentionally exercise expired
display metadata (`stale: true`), supported by the existing full
`verify_for_browse` policy. They do not authorize installation.

| Path | Catalog read | Whole successful request |
| --- | --- | --- |
| Unchanged | 1 primary D1 batch, 2 SELECTs/returned rows, 1 accepted R2 GET | Same; no acceptance write |
| Changed | 1 primary D1 batch, 2 SELECTs/returned rows, 2 R2 GETs | Also 1 missing-candidate R2 GET, 1 create-only R2 PUT and 1 D1 CAS write |

The changed case removes the candidate accepted object during setup so every
sample exercises the same acceptance operation. This table states assertions,
not measurements. It follows [W05's pre-acceptance counting boundary](workers-read-coalescing.md).
Returned R2 object sizes count bytes; a missing-object GET counts as a call with
zero bytes. D1's local `meta.rows_read` and `meta.rows_written` are recorded for
batch/run calls, along with returned rows. The `first()` API exposes returned rows
only. No replica/session API is introduced.

## Boundaries and evidence

Each of four size/path cells gets three new Miniflare/workerd processes and one
first HTTP sample per process. The first process also receives 32 further serial
samples. Normal retirement splits those into 30 samples in a warm Wasm generation
and two first samples after generation retirement. The harness checks that each
generation retires after exactly 16 admissions. Every HTTP request starts and
shuts down a fresh Rust Kernel App. The production eight-event admission window
and five-second event boundary are unchanged. The window is intentional: a
browser navigation requests the HTML, module, stylesheet, sample artwork and
catalog data concurrently. A value of one turns ordinary asset loading into
`503 host_unavailable` responses.

`artifactProcessStartupMs` measures externally from Miniflare construction to
readiness. `externalLatencyMs` measures Node's loopback HTTP fetch through full
body consumption, excluding fixture setup. They are separate distributions;
startup excludes the first HTTP request, which includes lazy initialization.
OS file caches are not flushed, and this is neither device-cold startup nor
deployed latency. Wall time is never labeled CPU.

JSON includes individual traces, sample counts, nearest-rank p50/p95/p99,
read-path and whole-event D1/R2 distributions, response byte counts and hashes,
Wasm linear memory at the event receipt, generation IDs, failures, Git HEAD and
working source hashes, lock/artifact identities, and tool/binary versions and
hashes. Wasm linear memory is neither total isolate memory nor a sampled peak.
The diagnostic headers and binding wrappers add local measurement overhead.
Small-sample p99 values describe this run and cannot establish a production
throughput or tail-latency budget. Empty distributions have zero samples and
null percentiles.

Outside the latency matrix, both sizes assert rollback, equivocation, untrusted
key, invalid signature, checkpoint corruption and R2 object-integrity rejection.
CAS probes schedule a real competing primary D1 update before the consumer CAS,
then assert a lost write, a fresh durable winner read and Rust revalidation for
newer, equivocating and identical winners. Each successful matrix sample also
compares the exact persisted checkpoint, token and envelope against the Rust
bootstrap result. Existing tests retain the eight-attempt contention bound and
cancellation/retirement coverage.

The local entry has no configured Cloudflare resource identifiers. Miniflare
creates only task-owned fixtures and denies outbound Worker fetches. The archived
G3 evidence and signed conformance fixture are unchanged. A successful build,
dry-run or local matrix does not prove deployment.

## Current execution status

The coordinator run passed all 140 timed samples and 26 rollback, signature,
trust, integrity and CAS assertions. The compact [local summary](archive/workers-d01/local.json)
contains the distributions and exact tool/artifact identities. The compressed
[raw receipt](archive/workers-d01/local.raw.json.gz) retains every trace with
portable path normalization; use `gzip -dc` to inspect it.

These measurements remain local Miniflare/workerd wall time on the recorded
machine. They do not establish Cloudflare production latency, CPU, total isolate
memory, fleet capacity or cost.
