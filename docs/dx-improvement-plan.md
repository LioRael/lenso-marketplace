# Marketplace developer experience plan

Status: revised proposal, 2026-09-19. Native-tool migration is implemented;
the isolated Stage 1 Workers SDK real-flow prototype is implemented, while
lifecycle qualification and any production host migration remain outstanding.

## Direction

Use Cloudflare's Rust Workers authoring experience as the baseline: write the
Worker entry and platform adapters in Rust, generate the JS/Wasm glue, and let
Wrangler own local serving, rebuilding, configuration and deployment.

The previous proposal prioritized converting handwritten JavaScript to TypeScript
and formalizing its JSON bridge. Those are no longer the starting assumptions.
First determine whether those application-owned layers can be removed entirely.
Keep native hosting as a supported alternative; use Workers as the reference
local development path for the Cloudflare deployment.

## Official baseline and verified sources

Cloudflare's [Rust guide](https://developers.cloudflare.com/workers/languages/rust/)
describes `workers-rs`, a Rust fetch handler, generated JavaScript glue, and
Wrangler-driven development and deployment. The official
[Rust entry template](https://github.com/cloudflare/workers-rs/blob/main/templates/hello-world/src/lib.rs)
uses `#[event(fetch)]` with Request, Env and Context. Its
[Wrangler template](https://github.com/cloudflare/workers-rs/blob/main/templates/hello-world/wrangler.toml)
points at generated `build/index.js` and invokes `worker-build` through a custom
build command. Rust changes rebuild during `wrangler dev`.

The SDK exposes [D1](https://github.com/cloudflare/workers-rs/blob/main/worker/src/d1/mod.rs)
and [R2](https://github.com/cloudflare/workers-rs/blob/main/worker/src/r2/mod.rs)
bindings to Rust. D1 includes prepared queries and batch execution. These sources
establish available integration seams, not equivalence with Marketplace's
conditional writes, cancellation, or instance-retirement guarantees. Repository
main is a moving reference: select and qualify exact released versions before
implementation, including Rust and wasm-bindgen compatibility.

## Current gap

Marketplace currently exposes `handle_http(String, JsValue)` through wasm-bindgen.
`http-host.mjs` assembles a custom runtime host, while `storage.mjs` owns D1/R2
transport and Rust calls it through string operations and JSON. The Worker's
Wrangler configuration points at handwritten `worker.mjs`; building Wasm is a
separate package command. This makes an application contributor maintain both
sides of platform glue that the official SDK may already cover.

The existing Runtime is more than glue: it provides admission limits, request
scopes, cancellation, bounded cleanup and Wasm generation retirement. Its host
requires generated reset support and manually instantiates Wasm. A standard
worker-build output is not proven to be a drop-in replacement for that lifecycle.

## Target ownership

| Concern | Intended owner |
| --- | --- |
| JS entry and wasm-bindgen glue | Official worker-build output |
| Local serve, Rust rebuild, bindings, deployment | Wrangler configuration |
| HTTP event and Cloudflare binding access | workers-rs SDK |
| D1/R2 implementations of Plugin storage ports | Rust Workers host adapters |
| Catalog, signatures, acceptance and publishing rules | Existing Rust Plugins |
| App composition, cancellation and cleanup | Lenso, where product requirements demand it |
| Native executable | Existing native App composition |
| Frontend HMR | Vite, connected to the local Workers backend |
| Profiling and browser automation | Explicit test tools; JS/TS is acceptable |

A conceptual host has a Rust fetch entry, Rust storage adapters and a Lenso App
composition. Its Wrangler `main` references generated output. Do not introduce a
new framework crate solely to hide code; use a private host module first and
extract only a demonstrated shared mechanism.

## 1. Prove one real flow on the official toolchain

Build a disposable local integration using pinned released `worker`, worker-build
and Wrangler versions. Keep the existing production host until comparison passes.
Wire the real Directory and Web Plugins and a signed local fixture, implement
Rust-backed D1/R2 published reads, and serve one real search/detail request.
A Hello World Worker alone is insufficient.

Put the build command and watch inputs in Wrangler configuration, including
workspace Plugin and contract sources. Invoke Wrangler from the documented
working directory. Verify the generated output location for this Cargo workspace
rather than copying the standalone template blindly. UI prerequisites must be
explicit; avoid rebuilding unchanged frontend assets on every Rust edit.

Acceptance:

- A standalone clone can run the flow through `pnpm exec wrangler dev` with the
  documented configuration and prerequisites, without a sibling checkout.
- Local D1/R2 fixtures require no production resources or publisher database.
- Editing the Rust entry or a consumed Plugin triggers rebuild and refresh.
- The prototype serves actual signed catalog data through the existing Plugins.
- Its generated JS is not hand-edited, and Wrangler dry-run bundles it.

## 2. Qualify the lifecycle before choosing the final host

Compare the current runtime with the official SDK integration. Exercise:

- Client disconnect, slow I/O and timeout during outstanding storage operations.
- Shutdown and cleanup of each request's App and request-owned bindings.
- Concurrent requests and stale continuations after cancellation.
- Panics/traps and subsequent request behavior.
- Repeated requests, Wasm memory growth and the reason for current retirement.
- Bounded body/object reads, read coalescing and conditional acceptance writes.

Separate externally required behavior from incidental implementation details.
The current retirement count is not automatically a permanent product contract,
but abandoning its resource/failure guarantees requires explicit evidence and a
reviewed replacement. Rust Futures being dropped does not by itself establish
that underlying platform I/O stopped or cannot settle later.

Decision: adopt the official entry/build path if it preserves the required
behavior. If a requirement needs additional machinery, identify the exact gap
and retain the narrowest Lenso integration necessary. If custom instantiation is
fundamental, document why stock worker-build cannot own the entry before designing
an extension. Do not pre-approve a second runtime or claim SDK equivalence.

## 3. Remove redundant application glue

Once stages 1 and 2 pass, move both published storage and accepted-cache transport
to Rust SDK adapters. Preserve primary D1 reads, batch consistency, content hashes,
create-only R2 writes, full pointer comparison and uncertain-write reconciliation.
Keep each Plugin's private storage port and authority unchanged.

Delete `storage.mjs` and the string-operation JSON callback protocol when no
production caller needs them. Replace or remove handwritten Worker/http entry
modules according to the selected lifecycle. Reuse the official SDK's FFI; do not
create a Marketplace code generator for a bridge that has disappeared.

Acceptance: search/detail, signature and corruption rejection, CAS contention,
read coalescing and lifecycle regression tests pass against local workerd. Move
tests with their owner instead of preserving obsolete JS interfaces for test
convenience. Retain only necessary platform glue, generated or explicitly justified.

## 4. Finish the contributor workflow

Make `pnpm dev` a thin alias to the documented Workers development workflow;
expose frontend HMR separately through Vite proxying to that local backend.
Keep `pnpm dev:ui` for the sample-only design view and an explicit native path.
Use Wrangler's local migration facilities and one narrow fixture-seeding command.
Do not build another long-running process supervisor just to wrap Wrangler.

The repository should expose a small command index for type/lint checks, native
Rust tests, Workers integration, browser acceptance and explicit profiling. Keep
`catalog`, `event-host` and `quality`; avoid repeated UI builds in aggregate checks.
Type-check surviving maintained Node tools, after deciding which survive.

Acceptance: a new contributor can start real local browsing, edit Rust and UI,
reset only disposable local data, and run focused checks from the README. Test
and performance tools remain discoverable without becoming startup prerequisites.

## 5. Review private publishing separately

workers-rs bindings execute inside Workers; they do not automatically replace the
protected operator's D1 REST/R2 S3 client. Keep promotion semantics, durable Rust
publisher state and signing authority intact. Evaluate a Rust CLI implementation
only against concrete maintenance benefits. Do not move publishing into a public
fetch route to eliminate a Node script. Node/TypeScript remains acceptable for
browser/proof orchestration and justified external operator tooling.

## Delivery and evidence

Implement the real-flow prototype first, then lifecycle qualification, deletion of
redundant glue, contributor commands and operator review. The first deliverable is
a compatibility decision with runnable evidence, not a bulk MJS-to-TS migration.

Measure comparable first-start time, Rust rebuild time, warm focused-test time,
artifact size, request latency and repeated-request memory on the same toolchain
and workload. Label cache conditions and local versus deployed evidence; no
performance budget or improvement is assumed. Preserve immutable previous receipts.

Native-tool retirement remains valid in a standalone checkout: use Cargo, Git,
GitHub CLI and Worktrunk directly. When working in the Lenso sibling workspace,
invoke Cargo through the framework-root `.lenso-tools/bin/lenso-cargo` as
required by repository policy. No sibling path discovery or private wrapper is
required by Marketplace. This plan changes the proposed Workers architecture,
not that completed tooling migration.
