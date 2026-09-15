#!/usr/bin/env bash
set -euo pipefail
: "${CARGO:?Set CARGO to cargo in CI or the local Lenso wrapper}"
export RUSTUP_TOOLCHAIN=1.94.0
cd "$(dirname "$0")/.."
pnpm build
for package in catalog directory web app; do
  "$CARGO" fmt --manifest-path "$package/Cargo.toml" --all -- --check
  "$CARGO" clippy --locked --manifest-path "$package/Cargo.toml" --workspace --all-targets -- -D warnings
  "$CARGO" test --locked --manifest-path "$package/Cargo.toml" --workspace
done
node catalog/tests/cross-language.mjs
