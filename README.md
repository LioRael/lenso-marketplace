# Lenso Marketplace

A Plugin marketplace built with Lenso. The canonical service stays at `marketplace.lenso.dev`. Its browsing UI and the Site read the keyless catalog's current head; the CLI verifies Sigstore publisher provenance before adopting an exact version. Browser display is not provenance verification or a plugin security review. Production cutover requires its own deployment and consumer receipts, separate from landed code and passing CI.

Normal adoption is `lenso app add PLUGIN_ID@VERSION --marketplace`. The managed verifier obtains its fixed official tool and public roots automatically; users do not maintain catalog signing keys, renew weekly signatures, or supply `--gh`/`--trusted-root` flags. See [keyless migration](docs/keyless-catalog-migration.md) for supported channels and delivery boundaries.

## Repository map

| Directory | Responsibility |
| --- | --- |
| `apps/native` | Native App composition and HTTP startup |
| `apps/workers` | Workers composition, D1/R2 adapters and public entrypoint |
| `apps/workers-sdk-prototype` | Disposable official `workers-rs` comparison host |
| `plugins/directory` | Catalog publishing, review/audit state and published-snapshot reads |
| `plugins/web` | Canonical browsing UI, legacy HTTP compatibility and consumer verification cache |
| `contracts/directory`, `contracts/linked-directory` | Separate generated read Capabilities for portable and source-only publications; no storage implementation |
| `tools/publisher` | Legacy signed-directory operator CLI and historical publication input |
| `tools/cloudflare` | Private publication promotion and deployment config generation |
| `tests` | Test plugins, protocol fixtures, integration and explicit Workers proof host |

Read [architecture](docs/architecture.md) for dependency direction and data flows.

## Publish a plugin

Follow the [author workflow](docs/publishing.md) to check, package and prepare a submission without Marketplace credentials. Existing publisher guides describe the legacy Ed25519 workflow; they remain historical operator references, not a requirement to introduce new signing keys or renewals. Keyless publication uses a reviewed exact catalog and the fixed GitHub attestation workflow.

## Build and verify

Use Node 24+, pnpm and Rust 1.94.0. The application has one Cargo workspace and lockfile, and one Node dependency lockfile. Test Plugin fixtures deliberately keep their own authoring workspaces. No Console source checkout is needed.

```sh
pnpm install --frozen-lockfile
pnpm build
pnpm test:native
pnpm test:browser
pnpm test:workers
pnpm build:workers
```

Browser acceptance needs Playwright Chromium and exercises the built v3 UI with a display-only fixture from the six reviewed records. It covers four channels, exact releases, filters, history, release states, current-head refresh failure/retry, keyboard navigation and mobile layout. It does not prove Sigstore verification or adoption. Workers builds need `wasm32-unknown-unknown` and wasm-bindgen-cli 0.2.127.

Foundation CI separately retains the real Echo Plugin's CLI check, execution and archive, native archive review and signed Directory API discovery using `bash scripts/verify-browser.sh --api-only`. Its original v1 GUI harness is a historical reference, not current browser acceptance. Set `LENSO_RUST_WORKSPACE` to an exact absolute Rust checkout to additionally run the archive-to-Rust-App proof; that cross-repository proof is not enabled in Marketplace CI.

## Run

For frontend development, `pnpm dev` serves the explicit `/?catalog=sample` design catalog. Real browsing requires the canonical v3 current head and hash-bound catalog object. Native v1 signed-publication commands below remain legacy paths.

```sh
cargo run --locked -p lenso-marketplace-app -- --help
cargo run --locked -p lenso-marketplace-publisher -- CONFIG.json verify < SNAPSHOT.json
```

Native Host arguments specify the publication database, catalog identity, public verification key and listen address. Publisher configuration and commands are in [the operator guide](tools/publisher/docs/operator.md).

The public Workers entry is `apps/workers/worker.mjs`. Its checked-in Wrangler config is a deployment template with no account, resource or signing identity. Provide a reviewed environment config with D1/R2 bindings and public trust, then use `wrangler dev --config ENV.json` or the explicit deployment procedure in [operations](docs/operations.md). It never exposes proof mutation routes.

Production configuration is rendered from explicit inputs rather than copied from the proof Worker. The renderer binds the target account, requires the `production` environment, and rejects proof/test/recovery identities and public proof trust. Keep the generated JSON outside version control and review its catalog ID, public-key fingerprint, migrations path, D1/R2 bindings and custom domain before deployment.

`pnpm dev:proof` explicitly starts the disposable test host in `tests/workers`. Its resource IDs, test key and diagnostics are not production defaults.

## Workers SDK prototype

The repository also contains an isolated official `workers-rs` comparison host. It is not the production entry and must not be used as a deployment configuration. The prototype pins `worker` and `worker-build` to `0.8.5`, composes the real Directory and Web Plugins, and uses only disposable local D1/R2 state with the committed public fixture.

Install the pinned build tool once, then run its complete local proof:

```sh
cargo install worker-build --version 0.8.5 --locked
pnpm test:workers:sdk
```

That command builds the UI, runs the Rust SDK host against local Miniflare bindings, and finishes with a Wrangler `deploy --dry-run`. It never creates or mutates Cloudflare resources. Wrangler watches the prototype, Directory, Web and Directory contract sources, so editing a consumed Rust Plugin rebuilds the generated Worker during `wrangler dev`.

For an interactive signed local session, seed Wrangler's disposable state and then start its dev server:

```sh
pnpm seed:workers:sdk
pnpm exec wrangler dev --config apps/workers-sdk-prototype/wrangler.jsonc
```

The prototype deliberately does not replace the current `apps/workers` host. Admission, cancellation fencing, generation retirement, and cleanup still require the lifecycle qualification described in [`workers-sdk-prototype`](docs/workers-sdk-prototype.md).
