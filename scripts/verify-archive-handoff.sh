#!/usr/bin/env bash
set -euo pipefail
: "${CARGO:=cargo}"
: "${1:?Provide the lenso Rust workspace checkout}"
: "${2:?Provide a CLI-built Echo archive}"
marketplace_root="$(cd "$(dirname "$0")/.." && pwd)"
proof_root="$(mktemp -d "${TMPDIR:-/tmp}/lenso-marketplace-handoff.XXXXXX")"
trap 'rm -rf "$proof_root"' EXIT
python3 - "$proof_root" "$marketplace_root" "$1" <<'PY'
from pathlib import Path
import json, sys
root, marketplace, lenso = map(lambda value: Path(value).resolve(), sys.argv[1:])
cli = lenso / 'crates/lenso-cli'
q = lambda value: json.dumps(str(value))
(root / 'Cargo.toml').write_text(f'''
[package]
name = "marketplace-archive-handoff-proof"
version = "0.0.0"
edition = "2024"
[workspace]
[lib]
path = {q(marketplace / 'tests/integration/archive_handoff.rs')}
[dependencies]
lenso-app-authoring = {{ package = "lenso-cli", path = {q(cli)}, default-features = false }}
lenso-marketplace-directory-plugin = {{ path = {q(marketplace / 'plugins/directory')} }}
lenso-plugin-catalog = {{ path = {q(lenso / 'crates/lenso-plugin-catalog')}, features = ["bundle-verification"] }}
tempfile = "3"
lenso-app-plan = "0.4.1"
ed25519-dalek = "2.2"
serde_json = "1"
[patch.crates-io]
lenso-cli = {{ path = {q(cli)} }}
[patch."https://github.com/LioRael/lenso.git"]
lenso-plugin-catalog = {{ path = {q(lenso / 'crates/lenso-plugin-catalog')} }}
''')
PY
MARKETPLACE_BUNDLE_ARCHIVE="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")" \
"$CARGO" test --manifest-path "$proof_root/Cargo.toml"
