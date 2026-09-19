#!/usr/bin/env bash
set -euo pipefail

# This script is deliberately separate from apps/workers/build.sh. The
# production Worker remains on the qualified Lenso runtime while this crate
# provides the disposable workers-rs comparison host.
root="$(cd "$(dirname "$0")/../.." && pwd)"
worker_build="${WORKER_BUILD:-worker-build}"

if ! command -v "$worker_build" >/dev/null 2>&1; then
  cat >&2 <<'EOF'
worker-build 0.8.5 is required for the Workers SDK prototype.
Install it with: cargo install worker-build --version 0.8.5 --locked
EOF
  exit 1
fi

version="$("$worker_build" --version)"
if [[ "$version" != "0.8.5" ]]; then
  printf 'Expected worker-build 0.8.5, found %s\n' "$version" >&2
  exit 1
fi

# worker-build resolves the workspace lock from the crate path. --locked is
# passed through to Cargo; --no-opt keeps this proof focused on SDK output and
# avoids an unpinned wasm-opt download. worker-build invokes `cargo` by name,
# so expose the repository-selected wrapper through a temporary PATH entry when
# the caller supplied one.
cargo_command="${CARGO:-cargo}"
shim_dir=""
cleanup() {
  if [[ -n "$shim_dir" ]]; then
    rm -rf "$shim_dir"
  fi
}
trap cleanup EXIT
if [[ "$cargo_command" != "cargo" ]]; then
  shim_dir="$(mktemp -d "${TMPDIR:-/tmp}/lenso-marketplace-sdk-cargo.XXXXXX")"
  cargo_path="$(command -v "$cargo_command")"
  ln -s "$cargo_path" "$shim_dir/cargo"
  export PATH="$shim_dir:$PATH"
fi

cd "$root/apps/workers-sdk-prototype"
"$worker_build" \
  --release \
  --locked \
  --no-opt \
  --out-dir build/worker
