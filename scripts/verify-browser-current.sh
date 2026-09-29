#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
pnpm build
node plugins/web/tests/keyless-display.mjs
