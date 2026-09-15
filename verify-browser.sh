#!/usr/bin/env bash
set -euo pipefail
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.94.0}"
: "${CARGO:?Set CARGO to the repository Cargo wrapper, or cargo in CI}"
marketplace_root="$(cd "$(dirname "$0")" && pwd)"
workspace_root="$(cd "$marketplace_root/../.." && pwd)"
proof_root="$(mktemp -d "${TMPDIR:-/tmp}/lenso-marketplace-browser.XXXXXX")"
server_pid=""
cleanup() {
  if [[ -n "$server_pid" ]]; then kill -INT "$server_pid" 2>/dev/null || true; wait "$server_pid" || true; fi
  rm -rf "$proof_root"
}
trap cleanup EXIT
cd "$workspace_root"
pnpm marketplace:build
"$CARGO" generate-lockfile --manifest-path "$marketplace_root/fixtures/echo/Cargo.toml"
lenso plugin pack --repo-root "$marketplace_root/fixtures/echo" --output "$proof_root/echo.lenso-plugin" --json
# Only the locally built test fixture is extracted here. Production archive
# ingestion remains gated on the released bounded archive API.
python3 - "$proof_root" <<'PY'
from pathlib import Path
from zipfile import ZipFile
import sys
root = Path(sys.argv[1])
with ZipFile(root / 'echo.lenso-plugin') as archive:
    archive.extractall(root / 'bundle')
PY
public_key="$(MARKETPLACE_BUNDLE_DIRECTORY="$proof_root/bundle" MARKETPLACE_BUNDLE_ARCHIVE="$proof_root/echo.lenso-plugin" "$CARGO" run --locked --manifest-path "$marketplace_root/catalog/Cargo.toml" --example seed_fixture -- "$proof_root/directory.sqlite3")"
"$CARGO" build --locked --manifest-path "$marketplace_root/app/Cargo.toml" --message-format=json-render-diagnostics > "$proof_root/build.jsonl"
server_binary="$(python3 - "$proof_root/build.jsonl" <<'PY'
import json,sys
for line in open(sys.argv[1]):
    item=json.loads(line)
    if item.get('reason') == 'compiler-artifact' and item.get('executable') and item['target']['name']=='lenso-marketplace-app':
        print(item['executable'])
PY
)"
"$server_binary" --app-root "$proof_root/app" --directory-database "$proof_root/directory.sqlite3" --catalog-id local-fixture --key-id test-key --public-key-hex "$public_key" --listen 127.0.0.1:0 > "$proof_root/server.log" 2>&1 &
server_pid=$!
server_url="$(python3 - "$proof_root/server.log" <<'PY'
import pathlib,re,sys,time
for _ in range(100):
    text=pathlib.Path(sys.argv[1]).read_text()
    match=re.search(r'listening on (http://127\.0\.0\.1:\d+)',text)
    if match:
        print(match[1]);break
    time.sleep(.1)
else:
    raise SystemExit('Host did not become ready: '+text)
PY
)"
MARKETPLACE_TEST_URL="$server_url" node "$marketplace_root/web/tests/browser.mjs"
