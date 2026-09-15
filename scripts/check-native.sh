#!/usr/bin/env bash
set -euo pipefail
: "${CARGO:?Set CARGO to cargo in CI or the local Lenso wrapper}"
export RUSTUP_TOOLCHAIN=1.94.0
cd "$(dirname "$0")/.."
pnpm build
"$CARGO" fmt --all -- --check
"$CARGO" clippy --locked --workspace --exclude lenso-marketplace-workers-host --all-targets -- -D warnings
"$CARGO" test --locked --workspace --exclude lenso-marketplace-workers-host
node tests/support/tests/cross-language.mjs
