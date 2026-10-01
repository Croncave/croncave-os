#!/usr/bin/env bash
# One command to run Croncave locally: Postgres, the control plane (with the relay and the
# preview edge), computers through the local driver, and the web app.
#
#   ./scripts/dev.sh          # http://localhost:5173
#   ./scripts/dev.sh --e2e    # a fresh database on other ports, for the Playwright tests
set -euo pipefail
. "$(dirname "$0")/lib.sh"
cd "$ROOT"
ensure_env

MODE=dev
[ "${1:-}" = "--e2e" ] && MODE=e2e

if [ "$MODE" = "e2e" ]; then
  export API_ADDR=127.0.0.1:18080 PREVIEW_ADDR=127.0.0.1:18081 PREVIEW_DOMAIN=preview.localhost:18081
  export WEB_URL=http://localhost:15173 RELAY_URL=http://127.0.0.1:18080 WEB_PORT=15173
  export DATA_DIR="$DEV_DIR/e2e-data" CRONCAVE_MOCK_CODER_PAUSE_MS=150
  DB=croncave_e2e
  rm -rf "$DATA_DIR"
  DATABASE_URL="" ensure_postgres postgres
  psql "postgres://croncave@127.0.0.1:$PG_PORT/postgres" -qc "drop database if exists $DB with (force)" -c "create database $DB" >/dev/null
fi
DATABASE_URL="${DATABASE_URL_OVERRIDE:-}" ensure_postgres "${DB:-croncave}"
export API_URL="http://${API_ADDR:-127.0.0.1:8080}"

say "Building the control plane and the agent"
cargo build --quiet -p croncave-server -p croncave-agent

if [ ! -d web/node_modules ]; then
  say "Installing the web app's packages"
  (cd web && pnpm install --frozen-lockfile --silent)
fi

pids=()
stop_all() {
  trap - EXIT INT TERM
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  # Computers are processes that outlive the control plane by design; stop them too.
  for f in "${DATA_DIR:-.dev/data}"/computers/*/agent.pid; do
    [ -f "$f" ] && kill -s TERM -- "-$(cat "$f")" 2>/dev/null || true
  done
  wait 2>/dev/null || true
}
trap stop_all EXIT INT TERM

./target/debug/croncave-server &
pids+=($!)
for _ in $(seq 1 100); do curl -fs "$API_URL/healthz" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fs "$API_URL/healthz" >/dev/null || die "The control plane didn't start"

if [ "$MODE" = "e2e" ]; then
  (cd web && pnpm run build >/dev/null && exec pnpm exec vite preview --strictPort) &
else
  (cd web && exec pnpm exec vite dev) &
fi
pids+=($!)

WEB="${WEB_URL:-http://localhost:5173}"
for _ in $(seq 1 300); do curl -fs "$WEB" >/dev/null 2>&1 && break; sleep 0.2; done
say "Croncave is running at $WEB"
say "Sign in with any email; read the link and codes at $WEB/dev (admin: ${ADMIN_EMAILS:-admin@croncave.local})"
wait -n "${pids[@]}"
