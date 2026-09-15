# Repository extraction

Source history was extracted from `LioRael/lenso-console`, subtree `plugins/marketplace`, at commit `ea3c5c356659c6cb589f1e810ddadb27fb93651e`. Original subtree history is retained with Git subtree split. Source commit IDs in old receipts remain Console commit IDs; frozen evidence is not rewritten.

The import includes Console PR #365 (stale browsing), plus the focused changes from PR #364 (publisher backups). Shared protocol changes remain owned by lenso-cli PR #340. The existing lenso-catalog-worker repository serves the legacy Module directory and is not this application.

This repository owns frontend dependencies, build fingerprints, native and Workers entrypoints, publisher utilities, schema, fixtures and CI. Console no longer owns or builds this subtree. No database migration, trust-key change, catalog identity change or production deployment is implied by extraction. Existing Cloudflare resource IDs remain unchanged. Operators must run commands from this repository and update any local deployment config whose entrypoint points into the former Console checkout.

## Extraction validation

From the standalone checkout: frontend typecheck/build and Ultracite pass;
all four native packages pass formatting, Clippy and tests, including consistent
publisher backup, stale browsing and strict installation verification. The real
Host browser suite passes navigation, expiry notice, saved state and narrow
layout checks. All 16 Workers storage/operator tests pass, including actual
workerd D1/R2 promotion. The deployable Wasm build and proof/public-entrypoint
Wrangler dry runs succeed. Cargo path dependencies resolve inside this repository.
No Console checkout is read by these build entrypoints.

The machine-local publisher-proof Wrangler config now points to this repository;
its previous config is retained beside it. This updates the next deployment's
source location only. Existing deployment receipts remain historical evidence.
