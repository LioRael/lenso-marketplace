#!/usr/bin/env bash
set -euo pipefail
export RUSTUP_TOOLCHAIN=1.94.0
cd "$(dirname "$0")/.."
pnpm build
cargo fmt --all -- --check
cargo clippy --locked --workspace --exclude lenso-marketplace-workers-host --all-targets -- -D warnings
cargo test --locked --workspace --exclude lenso-marketplace-workers-host
node tests/support/tests/cross-language.mjs
