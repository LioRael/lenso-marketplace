#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

# Keep the same UI prerequisite as the production Worker proof. The SDK host
# consumes the existing Plugin graph; it does not build a second frontend.
pnpm build
bash apps/workers-sdk-prototype/build.sh

# The test owns disposable Miniflare/workerd D1/R2 state and never contacts
# Cloudflare resources. RUN_WORKERS_SDK_FLOW is intentionally scoped here so
# the normal lightweight Workers unit suite does not require a wasm toolchain.
RUN_WORKERS_SDK_FLOW=1 node --test tests/workers/sdk-flow.test.mjs

# Prove that Wrangler rebuilds for the Rust entry and a consumed Plugin source,
# without watching the generated build output and causing a rebuild loop.
pnpm seed:workers:sdk
bash scripts/verify-workers-sdk-watch.sh

# Wrangler must be able to consume the generated worker-build output. This is a
# dry run only; the local-only config contains no account or deployment route.
pnpm exec wrangler deploy --dry-run \
  --config apps/workers-sdk-prototype/wrangler.jsonc
