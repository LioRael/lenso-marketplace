#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

port="${WRANGLER_SDK_PORT:-8791}"
log="$(mktemp "${TMPDIR:-/tmp}/lenso-marketplace-sdk-watch.XXXXXX")"
prototype_source="$root/apps/workers-sdk-prototype/src/lib.rs"
directory_source="$root/plugins/directory/src/event.rs"
prototype_reference="$(mktemp "${TMPDIR:-/tmp}/lenso-marketplace-sdk-source.XXXXXX")"
directory_reference="$(mktemp "${TMPDIR:-/tmp}/lenso-marketplace-sdk-directory.XXXXXX")"
server_pid=""

touch -r "$prototype_source" "$prototype_reference"
touch -r "$directory_source" "$directory_reference"

cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill -INT "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  touch -r "$prototype_reference" "$prototype_source" 2>/dev/null || true
  touch -r "$directory_reference" "$directory_source" 2>/dev/null || true
  rm -f "$log" "$prototype_reference" "$directory_reference"
}
trap cleanup EXIT

pnpm exec wrangler dev \
  --config apps/workers-sdk-prototype/wrangler.jsonc \
  --local \
  --no-show-interactive-dev-session \
  --port "$port" >"$log" 2>&1 &
server_pid=$!

for _ in {1..180}; do
  if grep -q "Ready on http://localhost:${port}" "$log"; then
    break
  fi
  if ! kill -0 "$server_pid" 2>/dev/null; then
    cat "$log"
    exit 1
  fi
  sleep 1
done
if ! grep -q "Ready on http://localhost:${port}" "$log"; then
  cat "$log"
  echo "Wrangler SDK prototype did not become ready" >&2
  exit 1
fi

check_response() {
  local response
  response="$(
    curl --fail --silent \
      "http://localhost:${port}/api/marketplace/v1/plugins?limit=1"
  )"
  node -e '
    const body = JSON.parse(process.argv[1]);
    if (
      body.catalog_id !== "workers-d01-local-only" ||
      body.revision !== 2 ||
      body.releases?.length !== 1
    ) {
      throw new Error("unexpected SDK prototype response");
    }
  ' "$response"
}

check_response

wait_for_rebuild() {
  local before_restart="$1"
  local before_done="$2"
  for _ in {1..90}; do
    local current_restart current_done
    current_restart="$(grep -c "restarting build" "$log" || true)"
    current_done="$(grep -c "Done in" "$log" || true)"
    if ((current_restart > before_restart && current_done > before_done)); then
      local completed_restart_count="$current_restart"
      sleep 2
      current_restart="$(grep -c "restarting build" "$log" || true)"
      if ((current_restart != completed_restart_count)); then
        cat "$log"
        echo "Wrangler continued rebuilding after the watched source settled" >&2
        return 1
      fi
      return 0
    fi
    sleep 1
  done
  cat "$log"
  echo "Wrangler did not complete a rebuild after a watched source change" >&2
  return 1
}

restart_count="$(grep -c "restarting build" "$log" || true)"
done_count="$(grep -c "Done in" "$log" || true)"
touch "$prototype_source"
wait_for_rebuild "$restart_count" "$done_count"
check_response

sleep 2
restart_count="$(grep -c "restarting build" "$log" || true)"
done_count="$(grep -c "Done in" "$log" || true)"
touch "$directory_source"
wait_for_rebuild "$restart_count" "$done_count"
check_response
