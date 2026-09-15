# W05 Marketplace Workers catalog reads

Browse, search and detail retain the Web Plugin's Plan-bound Directory read.
The Web Plugin gives its private event storage a one-shot catalog-read hint.
The Workers Directory storage consumes that hint and invokes `catalog`, which
returns publication bytes and the observed accepted state together. Raw snapshot
requests continue to use `published` independently of accepted state.

## Durable reads

The operation reads publication and accepted pointers with two SELECT statements
in **one primary D1 batch**, then authenticates the accepted R2 object's full
immutable key, including checkpoint bytes, pointer token and exact envelope digest.
Only when the current publication digest equals that verified token does it reuse
the accepted envelope. Otherwise it reads and independently digest-checks the
publication R2 object. JS enforces storage integrity; Rust owns catalog trust.

| State | D1 calls | R2 GET calls before acceptance |
| --- | --- | --- |
| Unchanged exact envelope | 1 batch (2 SELECTs) | 1 accepted object |
| Changed exact envelope | 1 batch (2 SELECTs) | 1 accepted + 1 publication object |
| Missing accepted pointer | 1 batch (2 SELECTs) | 1 publication object |
| Missing publication pointer | 1 batch (2 SELECTs) | 1 accepted object, if present |
| Both pointers absent | 1 batch (2 SELECTs) | 0 |

These counts exclude the unchanged acceptance write protocol when a new catalog
must be persisted. They are deterministic adapter call counts, not deployed
latency measurements.

Publication keys retain their existing opaque-key contract, including archived
`proof/name.json` fixtures. Pointer shape and SHA-256 syntax are checked; fetched
publication bytes must match the pointer digest. Reuse authenticates those bytes
through the accepted object and matching publication digest without fetching the
publication object. Accepted keys remain content-addressed over the complete
stored JSON at `accepted/<catalog>/<state digest>.json`.

## Trust, races and lifetime

The host hands the observed accepted state to Rust once, preserving the distinction
between an absent state and a read not yet performed. Rust still verifies
signature, configured trust, checkpoint, rollback and equivocation before its
exact-envelope early return or compare-and-swap. Every lost CAS triggers a fresh
accepted-pointer/object read and winner revalidation. The existing eight-attempt
bound, expected-token fence and immutable-object write protocol remain in place.

The handoff belongs to one HTTP event, is keyed to its catalog, and is consumed by
the first acceptance read. Starting another catalog read discards any prior
handoff. It provides no TTL or state shared between requests. Errors remain
errors; failed or malformed D1 results cannot become missing accepted state.
Missing publication is returned separately from prior accepted state so the
existing Directory-unavailable handling remains in Rust. Runtime cancellation,
late-callback fencing and R2 reader cleanup also cover the combined operation.

## Validation on the W05 source

The committed cross-language signed vector and archived Workers evidence are
unchanged. New tests cover exact-byte call counts, missing state, corrupt pointers,
checkpoint and envelope objects, bounds, cancellation, fresh winner reads and
Rust trust/CAS regression behavior.

Coordinator validation passed with the frozen repository dependencies:

- `pnpm build`;
- all 40 `pnpm test:workers` checks, including the workerd promotion fixture;
- all 10 `lenso-marketplace-web-plugin` unit and integration tests;
- Rust 1.94 wasm32 Clippy with warnings denied;
- the Workers release build and Wrangler deployment dry run; and
- `git diff --check`.

Required `catalog`, `event-host` and `quality` delivery checks remain unchanged.
These local checks and a dry run do not prove a new deployment.
