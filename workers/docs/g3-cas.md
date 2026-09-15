# G3 actual binding contention proof

This closes a specific evidence gap: the full smoke's 12-request burst runs
after an envelope is accepted and therefore takes `EventCache`'s identical-byte
fast path. It proves admission behavior, not competing D1 writes. The native
deterministic race proves re-verification against a winning checkpoint; this
additional proof exercises the actual Workers storage adapter and real D1/R2.

The public `worker.mjs` remains unchanged. Disposable
[`proof-worker.mjs`](../proof-worker.mjs) delegates every path except exact
`/__proof/cas` to it. The proof route requires POST, a hashed comparison against
the `G3_PROOF_KEY` Bearer secret, explicit G3 catalog/resource guards, and the
SHA-256 of a reviewed Rust-generated fixture. Public deployment configuration
must continue using `worker.mjs`, which has no CAS proof route.

## Prepare a verified fixture without changing D1/R2

After generating the normal G3 fixtures, run from the Console checkout:

```sh
/Users/leosouthey/Projects/framework/.lenso-tools/bin/lenso-cargo +1.94.0 run \
  --locked --manifest-path plugins/marketplace/catalog/Cargo.toml \
  --example workers_race_fixtures -- \
  /absolute/path/to/g3-fixtures /absolute/path/to/new-cas-fixtures
```

The [Rust example](../../catalog/examples/workers_race_fixtures.rs) uses the
existing deterministic G3 test key, never a production key. It verifies `first`
revision 1, `large` revision 4, then `restored` revision 5 with the shared Rust
protocol. Including `large` preserves all 161 historical identities even though
the current restored snapshot is empty. It signs and verifies distinct revisions
6 and 7 against that checkpoint, and verifies revision 8 after either winner.
The output contains `race.json` and exact signed `restored-8.json`; existing output
files are refused. The summary includes fixture SHA-256 and expiry, without key
material. Regenerate source and race fixtures if they expire; do not edit dates
or replace the digest to bypass verification.

The race JSON includes the complete expected state and two `AcceptedEnvelope`
candidates. The wrapper checks its exact configured hash and proof validity
window. JavaScript does not implement signature verification or invent a
checkpoint: the approved bytes come from the shared Rust verifier.

## Configure only the disposable proof deployment

The deployment owner must explicitly switch the G3 proof config's `main` to
`proof-worker.mjs`, install the secret through the approved secret channel, and
set these non-secret proof variables:

| Variable | Required value |
| --- | --- |
| `CATALOG_ID` | `workers-g3-proof` |
| `PROOF_DIAGNOSTICS` | `1` |
| `G3_PROOF_DATABASE_ID` | `2b913921-3f0a-43b4-ae8f-cb219c21db81` |
| `G3_PROOF_BUCKET_NAME` | `lenso-marketplace-g3-proof` |
| `G3_CAS_FIXTURE_SHA256` | Exact lowercase SHA-256 emitted for `race.json` |

`G3_PROOF_KEY` must be 32–256 characters and supplied only as a Worker secret and
the operator process environment. Never include it in arguments, the fixture,
repository, receipt or logs. The wrapper uses the Authorization header and does
not echo it. It returns only fixed failure codes and public fixture metadata.

Runtime binding objects do not expose their configured database UUID or bucket
name. The wrapper's guard variables therefore accompany, rather than prove, the
binding identity. Before deployment, verify that `MARKETPLACE_DB` and
`MARKETPLACE_OBJECTS` actually target the UUID/bucket above. The client script
independently checks the local proof config's Worker, entrypoint, variables and
binding IDs before any mutation. No production config uses this wrapper.

## Run after the complete smoke has finished

Do not overlap this probe with the full smoke or another mutation. The normal
smoke must leave both publication and accepted state at restored revision 5.
The client verifies that prerequisite through read-only D1/R2 calls; it will not
reset a pointer, overwrite an object or invent a baseline to make the test run.

From `plugins/marketplace/workers`, with `G3_PROOF_KEY` already supplied securely:

```sh
node proof/consumers-race.mjs \
  https://lenso-marketplace-g3-proof.lenso.workers.dev \
  /absolute/path/to/cas-fixtures/race.json --remote \
  /absolute/path/to/new-cas-receipt.json
```

For local binding qualification, use an explicit localhost origin and `--local`.
The remote script only accepts the dedicated G3 workers.dev origin. It reserves
a new receipt file before the POST and refuses to overwrite prior evidence.
It sends exactly one POST and never retries an uncertain outcome.

Within one request-owned scope, the wrapper invokes actual
`createStorage(...).compare_exchange` promises concurrently with the same
revision-5 token and distinct candidate tokens. Both candidates have already
been Rust-verified and include the full historical checkpoint. Exactly one
initial write must return true and the other false. The wrapper then reads the
durable winner through the adapter, repeats the loser with the stale token, and
requires false plus an unchanged winner. Existing R2 bytes remain exact-match
checked; missing objects are still create-only. The proof does not weaken CAS,
make an uncertain write successful, or add retries.

The response records before/after/final D1 pointer, R2 key/size/etag, history digest,
both CAS results and the stale result. The client independently reads the pointer
and whole R2 object before and after, checks its content-addressed key and envelope
token, and compares the final state with the exact Rust-verified winning candidate.
The saved receipt retains those durable reads. This proves the adapter's real
losing-write result; it is not a throughput benchmark or a cross-isolate load test.

Input is limited to 128 KiB with a five-second body-read deadline. The operation
has a ten-second proof watchdog, uses the existing request-owned storage scope,
waits for both submitted writes to settle on ordinary failure, and bounds cleanup
at 250 ms. On timeout or uncertain settlement, it returns 503, invalidates late
continuations and requires durable-state inspection. D1/R2 writes already issued
may have committed; cancellation does not roll them back.

## Preserve evidence and restore normal proof reads

The race intentionally advances accepted state to revision 6 or 7 while the
publication remains revision 5. Normal verified browse will reject that older
publication until the operator publishes the generated `restored-8.json` through
the proof-only publication procedure. Preserve the CAS receipt first, then upload
the exact revision-8 envelope and conditionally advance the publication pointer
from its observed revision 5 to 8. Verify exact snapshot bytes and successful
shared Web reads. Do not lower or clear accepted history to restore availability.

The script does not perform that publication or any cleanup automatically. A
failed or interrupted POST may have changed accepted state; inspect D1 and the
referenced immutable R2 object before deciding what to do. Once qualification is
complete, restore the proof config entrypoint to `worker.mjs` or retire the entire
disposable proof deployment and its secret under the owner's cleanup plan.

Current implementation checks establish syntax and fixture generation only.
Record local/deployed execution receipts and exact Worker version after the owner
authorizes and performs the probe; do not call this real-D1 proof passed in advance.
