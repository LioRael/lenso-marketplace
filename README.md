# Lenso Marketplace

A Plugin marketplace built with Lenso. Directory owns published catalog data;
Web owns browsing. Native and Cloudflare Workers are two hosts for the same
Plugins. Console Agent owns installation and consumes the signed catalog API.

## Repository map

| Directory | Responsibility |
| --- | --- |
| `apps/native` | Native App composition and HTTP startup |
| `apps/workers` | Workers composition, D1/R2 adapters and public entrypoint |
| `plugins/directory` | Catalog publishing, review/audit state and published-snapshot reads |
| `plugins/web` | Search/detail HTTP, Lenso UI frontend and consumer verification cache |
| `contracts/directory` | Generated Directory Capability; no storage implementation |
| `tools/publisher` | Local operator CLI for Directory publishing and backup |
| `tools/cloudflare` | Private publication promotion and deployment config generation |
| `tests` | Test plugins, protocol fixtures, integration and explicit Workers proof host |
| `docs/archive` | Historical designs and qualification evidence, not current runbooks |

Read [architecture](docs/architecture.md) for dependency direction and data flows.

## Build and verify

Use Node 24+, pnpm and Rust 1.94.0. The application has one Cargo workspace and
lockfile, and one Node dependency lockfile. Test Plugin fixtures deliberately keep
their own authoring workspaces. No Console source checkout is needed.

```sh
pnpm install --frozen-lockfile
pnpm build
CARGO=cargo pnpm test:native
CARGO=cargo pnpm test:browser
pnpm test:workers
CARGO=cargo pnpm build:workers
```

In the Lenso sibling workspace, replace `CARGO=cargo` with the absolute path to
`.lenso-tools/bin/lenso-cargo`. Browser acceptance needs `lenso-cli` and Playwright
Chromium. Workers builds need `wasm32-unknown-unknown` and wasm-bindgen-cli 0.2.127.

## Run

For frontend development, `pnpm dev` serves the explicit `/?catalog=sample` design
catalog. Real data requires a Host and a signed publication.

```sh
cargo run --locked -p lenso-marketplace-app -- --help
cargo run --locked -p lenso-marketplace-publisher -- CONFIG.json verify < SNAPSHOT.json
```

Native Host arguments specify the publication database, catalog identity, public
verification key and listen address. Publisher configuration and commands are in
[the operator guide](tools/publisher/docs/operator.md).

The public Workers entry is `apps/workers/worker.mjs`. Its checked-in Wrangler
config is a deployment template with no account, resource or signing identity.
Provide a reviewed environment config with D1/R2 bindings and public trust, then
use `wrangler dev --config ENV.json` or the explicit deployment procedure in
[operations](docs/operations.md). It never exposes proof mutation routes.

`pnpm dev:proof` explicitly starts the disposable test host in `tests/workers`.
Its resource IDs, test key and diagnostics are not production defaults.
