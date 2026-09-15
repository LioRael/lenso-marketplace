# G3 Marketplace event-host qualification

The Marketplace Directory and Web Plugins use their normal generated contracts,
resolved Plugin Root and portable signed catalog protocol. Native SQLite remains
the default. The Workers host injects private D1/R2 storage and an event clock;
no signing private key or Agent installation connection is added to public reads.

The proof is restricted to `lenso-marketplace-g3-proof` resources and deterministic
synthetic trust. It is not an official catalog, production deployment, or an
unrestricted Workers capacity claim.

## Covered behavior

- Exact signed bytes, native persisted-cache replay and Workers verification.
- Same-revision equivocation, invalid signature, expiry and rollback rejection.
- Immutable release identity after omission; full accepted-checkpoint object
  integrity, including historical identity entries.
- Durable checkpoints across Wasm generation retirement, storage failure and
  recovery without lowering the rollback fence.
- Bounded admission and a 3,609,427-byte signed catalog, including exact raw response.
- Event cancellation, bounded uncertain native-I/O settlement and stale callback
  fencing. A cancelled read still releases a body returned by late R2 completion;
  an absent-object lookup cannot begin a new write after cancellation.

A JSON byte-array response initially grew Wasm memory above 226 MiB and failed on
the deployed Worker. Canonical bounded Base64 at the Host response boundary
removed that allocation amplification. Final measurements report **Wasm linear
memory**, not total isolate peak memory. The profile admits one active event per
isolate, retires at 16 admissions, uses a five-second event boundary and limits
request/response bodies to 64 KiB/4 MiB. These are qualified proof constraints,
not an approved production throughput budget.

A repeated create-only upload of an existing large R2 object returned a network
failure. The adapter now reads and compares existing immutable bytes first. If
absent, creation remains conditional; a losing conditional create must match
exact bytes. The D1 compare-and-swap always runs. Unknown write outcomes are
reported as failures and never blindly retried or treated as rolled back.

The current Kernel retires the Directory provider and its consumer on a storage
PluginFailure. Therefore a storage outage can produce 503 despite a persisted
checkpoint; this proof does not promise a warm-cache 200. The preserved checkpoint
still rejects stale or altered releases after recovery.

## Reproduction

Use the [pinned cohort bootstrap](../docs/cohort-bootstrap.md), build the existing
Marketplace UI assets, then run `CARGO="${LENSO_CARGO:-cargo}" bash build.sh` from this
Workers directory with Rust 1.94.0 and wasm-bindgen CLI 0.2.127. Install the locked
pnpm dependencies before Wrangler. Apply the explicit migration to dedicated
proof resources; never run the destructive smoke against production data.

Generate fixtures with the catalog `workers_fixtures` example and replay them with
`workers_native_replay`. `proof/smoke.mjs` runs the public-read proof locally or
remotely. `node --test storage.test.mjs storage-scope.test.mjs` covers private
adapter cancellation and continuation ownership. The separate CAS proof records
real competing D1 writes; see its dedicated instructions.

Production domain, signing ownership, renewal, paired-state backup and forward-only
recovery requirements are in [G5 preflight](../docs/g5-preflight.md) and the
[rollout runbook](../docs/g5-rollout.md). Public production configuration is rendered
by `proof/deployment-config.mjs`; it rejects the known proof trust and legacy
catalog hostname and performs no deployment. No production catalog or registry
package has been published by this qualification.


The final proof harness uses D1 query calls for small fixture mutations. A prior
bulk SQL-file import lost its management connection before the first publication
committed; a read-back showed both proof pointer tables empty. That attempt is not
a passing receipt and no library write was blindly replayed. The isolated suite
was explicitly restarted after reconciliation.


## Final receipts

The clean source build is deployed as Worker version
`60c5f58b-77af-41e1-bdea-0bd6837ec8ce`. The [local](local.json) and
[remote](remote.json) public-read suites each pass 127 checks. The exact signed
fixtures also pass the [native SQLite replay](native.json), and the existing
[real Host/browser flow](native-browser.json) remains green. The
[isolate continuity](isolate-continuity.json) receipt separately proves that a new
Worker boot reads revision 5 from the retained bindings; its earlier deployment
identity is preserved in that receipt.

The [source cohort](cohort.json) records clean exact commits. The
[artifact manifest](artifact.json) records the clean-build file hashes and the
independently unpacked deployment archive. Its public entry excludes the
secret-guarded CAS probe and all test signing source. Rebuilds at different absolute
paths can have different Rust/wasm-bindgen symbol identities; byte-for-byte
reproducibility across paths is not claimed. Deploy and review the frozen artifact.


The [real D1 CAS receipt](cas.json) has one winner and one losing write; repeating
the losing candidate with the stale token remains false, and an independent D1/R2
read confirms the winning state and all 161 historical identities. The
[forward restoration](restoration.json) then publishes revision 8 conditionally
and verifies the public read. It never resets the consumer checkpoint.

## Source follow-up validation

The subsequent repository-style cleanup preserves the frozen archive and all raw
receipts above. It formats JavaScript, extracts the unchanged compare-and-swap
operation, and makes the proof's cleanup response explicit after `finally`. It
does not replace the historical deployment identity or claim those bytes were
redeployed. The six focused storage/lifecycle checks and repository lint pass
on the follow-up source. Console's 260 unit tests, 83 browser tests, distribution
checks, production build and contract type checks also pass.

The [paired consumer recovery](../recovery/RESULTS.md) separately deploys the
frozen public entry and verifies restoration, rollback rejection and corrupted
checkpoint recovery without changing the original source state.

The current bootstrap additionally pins Auth `f63bad90ac28b71d7aa5152a3ce94031577ffd9f`, whose final G4 deployment fences pending outbound Fetch callbacks. The archived clean-build cohort above remains unchanged historical evidence; Auth is not linked into the anonymous Marketplace public Worker. G4 records 59 remote checks and eight local lifecycle/package tests after independent review.
