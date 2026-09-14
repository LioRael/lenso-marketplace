#!/usr/bin/env bash
set -euo pipefail
: "${CARGO:?Set CARGO to the repository Cargo wrapper}"
marketplace_root="$(cd "$(dirname "$0")" && pwd)"
proof_root="$(mktemp -d "${TMPDIR:-/tmp}/lenso-marketplace-proof.XXXXXX")"
trap 'rm -rf "$proof_root"' EXIT
fixture_root="$marketplace_root/fixtures/echo"
manifest="$marketplace_root/catalog/Cargo.toml"
"$CARGO" test --locked --manifest-path "$manifest"
node "$marketplace_root/catalog/tests/cross-language.mjs"
lenso plugin check --repo-root "$fixture_root"
lenso plugin dev --repo-root "$fixture_root" --operation execute \
  --request-json '{"name":"lenso.marketplace.echo","arguments_json":"{\"text\":\"marketplace proof\"}"}'
lenso plugin pack --repo-root "$fixture_root" --output "$proof_root/echo.lenso-plugin" --json
# Test-only extraction of the archive just built and verified by CLI above.
# Production ingestion must use the installation owner's bounded archive API.
python3 - "$proof_root" <<'PY'
from pathlib import Path
from zipfile import ZipFile
import sys
root = Path(sys.argv[1])
with ZipFile(root / 'echo.lenso-plugin') as archive:
    archive.extractall(root / 'bundle')
PY
MARKETPLACE_BUNDLE_DIRECTORY="$proof_root/bundle" \
MARKETPLACE_BUNDLE_ARCHIVE="$proof_root/echo.lenso-plugin" \
"$CARGO" test --locked --manifest-path "$manifest" --test real_bundle -- --ignored
