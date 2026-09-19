# Official Workers SDK prototype

Status: Stage 1 real-flow prototype. The production Worker still uses
`@lenso/workers-runtime`; this host is not a deployment replacement.

## Purpose

The prototype answers one narrow question from the developer-experience plan:
can Cloudflare's released Rust SDK expose the existing Marketplace Plugin graph
against real local D1/R2 bindings without a sibling checkout or handwritten
JavaScript storage bridge?

It uses the same Web ingress, Directory and Web Plugins as the production host.
The Rust entry is `#[event(fetch)]` and obtains `MARKETPLACE_DB`,
`MARKETPLACE_OBJECTS`, catalog identity and public trust from Workers bindings.
Its private storage adapter implements:

- primary D1 batch reads for publication and accepted pointers;
- bounded R2 reads and publication digest verification;
- content-addressed accepted checkpoints;
- create-only R2 writes;
- complete-pointer D1 compare-and-swap with exact durable reconciliation after
  an uncertain write; and
- the one-shot catalog-read handoff used by the Web Plugin.

The product Plugins remain the authority for signatures, trust, rollback and
catalog admission. The prototype only translates the Workers bindings into
their existing private storage ports.

## Pinned toolchain

| Tool | Version |
| --- | --- |
| Rust package `worker` | `=0.8.5` |
| `worker-build` | `0.8.5` |
| Wrangler | `4.107.0` |
| Miniflare/workerd fixture runner | `4.20260701.0` |
| Rust target | `wasm32-unknown-unknown` |

The build script verifies `worker-build` before invoking it and passes `--locked`
to Cargo. It also honors `CARGO` by exposing a caller-selected repository
wrapper temporarily, because `worker-build` invokes Cargo by executable name.
The generated `build/worker/index.js`, Wasm module and compatibility
`build/worker/shim.mjs` are ignored build output; no generated file is edited.

Wrangler's custom build watches the source subdirectories (not generated
`build/` output, which would otherwise trigger its own rebuild) for:

- `apps/workers-sdk-prototype`;
- `plugins/directory`;
- `plugins/web`; and
- `contracts/directory`.

## Local proof

From the repository root:

```sh
cargo install worker-build --version 0.8.5 --locked
CARGO=cargo pnpm test:workers:sdk
```

The focused command:

1. builds the embedded UI once;
2. builds the SDK prototype from the root Cargo lock;
3. starts Miniflare/workerd with task-owned D1/R2 persistence;
4. applies the checked-in public-read migration;
5. writes the signed, committed local fixture to local R2 and its publication
   pointer to local D1;
6. requests `/api/marketplace/v1/plugins?limit=1` through the generated Rust
   Worker and checks catalog identity, revision, total and release bytes;
7. seeds the same signed fixture into Wrangler's disposable local D1/R2 state;
8. starts `wrangler dev` and checks a signed browse response before and after a
   Rust entry rebuild and a consumed Directory Plugin rebuild, with one
   completed rebuild and no generated-output rebuild loop for each; and
9. asks Wrangler to bundle the same generated output with `--dry-run`.

The fixture contains only a public verification key and signed bytes. No
publisher database, signing key, production resource ID or network fetch is
needed. The lightweight `pnpm test:workers` suite also runs the prototype's
toolchain/configuration assertions, while the real Miniflare flow runs only
through `pnpm test:workers:sdk`.

For an interactive local session after the fixture proof, run from the
repository root:

```sh
pnpm seed:workers:sdk
pnpm exec wrangler dev --config apps/workers-sdk-prototype/wrangler.jsonc
```

The seed command applies the checked-in migration and writes the public signed
fixture to Wrangler's local D1/R2 state only. Then browse
`http://localhost:8787/api/marketplace/v1/plugins?limit=1` while editing the
Rust entry or a watched Plugin source. It never addresses a remote resource.

## Deliberate non-goals

This prototype is evidence for the real-flow seam, not SDK equivalence. It
does **not** yet claim:

- bounded concurrent event admission or generation retirement;
- disconnect cancellation and late D1/R2 completion fencing;
- equivalent panic/trap recovery and subsequent-request behavior;
- bounded cleanup when platform I/O cannot be forcibly cancelled; or
- replacement of `apps/workers/http-host.mjs`, `storage.mjs`,
  `storage-scope.mjs` or the production Wrangler configuration.

Those behaviors remain covered by the existing host and must be compared on
identical local workerd fixtures before deleting any current glue. The next
decision gate is a lifecycle matrix covering disconnect, timeout, cancellation,
shutdown, repeated requests, memory growth, read coalescing and CAS races. A
successful prototype request or Wrangler dry-run is not a deployment receipt
and is not permission to switch production hosting.
