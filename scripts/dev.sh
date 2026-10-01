#!/usr/bin/env bash
# One command to run Croncave locally: Postgres, the control plane (with the relay and the
# preview edge), computers through the local driver, and the web app.
#
#   ./scripts/dev.sh          # http://localhost:5173
#   ./scripts/dev.sh --e2e    # a fresh database on other ports, for the Playwright tests
#   ./scripts/dev.sh --docker # computers are Docker containers (COMPUTE_DRIVER=docker)
#
# While it runs, changes are picked up by themselves (so a `git pull` is all it takes): the
# web app reloads in the browser, Rust changes rebuild and restart the control plane, and a
# changed web lockfile reinstalls packages. Computers keep running across a restart; they
# get a rebuilt agent the next time they wake. --no-watch turns this off.
set -euo pipefail
. "$(dirname "$0")/lib.sh"
cd "$ROOT"
ensure_env

MODE=dev
WATCH=1
for arg in "$@"; do
  case "$arg" in
    --e2e) MODE=e2e WATCH=0 ;;
    --docker) export COMPUTE_DRIVER=docker ;;
    --no-watch) WATCH=0 ;;
    *) die "Unknown option $arg (use --e2e, --docker or --no-watch)" ;;
  esac
done

# Computers outlive the control plane by design (work never depends on it). Stop the ones
# this data folder's last run left, if it was stopped too hard to clean up.
stop_computers() {
  local data="${DATA_DIR:-.dev/data}"
  for f in "$data"/computers/*/agent.pid; do
    [ -f "$f" ] && kill -s TERM -- "-$(cat "$f")" 2>/dev/null || true
  done
  if command -v docker >/dev/null 2>&1 && [ -d "$data" ]; then
    local base ids
    base="$(cd "$data" && pwd)/computers"
    ids="$(docker ps -aq --filter "label=croncave.base=$base" 2>/dev/null || true)"
    [ -n "$ids" ] && docker rm -f $ids >/dev/null 2>&1 || true
  fi
}

if [ "$MODE" = "e2e" ]; then
  export API_ADDR=127.0.0.1:18080 PREVIEW_ADDR=127.0.0.1:18081 PREVIEW_DOMAIN=preview.localhost:18081
  export WEB_URL=http://localhost:15173 RELAY_URL=http://127.0.0.1:18080 WEB_PORT=15173
  export DATA_DIR="$DEV_DIR/e2e-data" CRONCAVE_MOCK_CODER_PAUSE_MS=150
  DB=croncave_e2e
  stop_computers
  rm -rf "$DATA_DIR"
  DATABASE_URL="" ensure_postgres postgres
  psql "postgres://croncave@127.0.0.1:$PG_PORT/postgres" -qc "drop database if exists $DB with (force)" -c "create database $DB" >/dev/null
fi
DATABASE_URL="${DATABASE_URL_OVERRIDE:-}" ensure_postgres "${DB:-croncave}"
export API_URL="http://${API_ADDR:-127.0.0.1:8080}"

if [ "${COMPUTE_DRIVER:-local}" = "docker" ]; then
  ./scripts/build-computer-image.sh
  # Containers reach the host as host.docker.internal. On Linux that is the bridge's
  # gateway, so the relay (and the demo sites, never the API) also listen there.
  api_addr="${API_ADDR:-127.0.0.1:8080}"
  api_port="${api_addr##*:}"
  relay_port=$((api_port + 10))
  if [ "$(uname -s)" = "Linux" ]; then
    gateway="$(docker network inspect bridge -f '{{(index .IPAM.Config 0).Gateway}}')"
    export RELAY_ADDR="$gateway:$relay_port"
    export RELAY_URL="http://host.docker.internal:$relay_port"
  else
    export RELAY_URL="http://host.docker.internal:$api_port"
  fi
  export DEMO_URL="$RELAY_URL"
  say "Computers run as Docker containers from ${COMPUTER_IMAGE:-croncave-computer:dev}"
fi

say "Building the control plane and the agent"
cargo build --quiet -p croncave-server -p croncave-agent

if [ ! -d web/node_modules ]; then
  say "Installing the web app's packages"
  (cd web && pnpm install --frozen-lockfile --silent)
fi

SERVER=""
WEBPID=""
stop_all() {
  trap - EXIT INT TERM
  kill $SERVER $WEBPID 2>/dev/null || true
  stop_computers
  wait 2>/dev/null || true
}
trap stop_all EXIT INT TERM

start_server() {
  ./target/debug/croncave-server &
  SERVER=$!
  for _ in $(seq 1 100); do curl -fs "$API_URL/healthz" >/dev/null 2>&1 && return 0; sleep 0.1; done
  return 1
}

start_web() {
  if [ "$MODE" = "e2e" ]; then
    (cd web && pnpm run build >/dev/null && exec pnpm exec vite preview --strictPort) &
  else
    (cd web && exec pnpm exec vite dev) &
  fi
  WEBPID=$!
}

start_server || die "The control plane didn't start"
start_web
WEB="${WEB_URL:-http://localhost:5173}"
for _ in $(seq 1 300); do curl -fs "$WEB" >/dev/null 2>&1 && break; sleep 0.2; done
say "Croncave is running at $WEB"
say "Sign in with any email; read the link and codes at $WEB/dev (admin: ${ADMIN_EMAILS:-admin@croncave.local})"

if [ "$WATCH" = "0" ]; then
  wait -n $SERVER $WEBPID
  exit
fi

say "Watching for changes (Ctrl-C to stop)"
STAMP="$DEV_DIR/.watch-stamp"
touch "$STAMP"
changed() {
  [ -n "$(find crates Cargo.toml Cargo.lock -newer "$STAMP" -type f ! -path '*/target/*' -print -quit 2>/dev/null)" ]
}
while true; do
  sleep 1.5
  # Stop if either half stopped by itself.
  kill -0 $WEBPID 2>/dev/null || die "The web app stopped"
  if [ web/pnpm-lock.yaml -nt "$STAMP" ]; then
    touch "$STAMP"
    say "The web app's packages changed; reinstalling"
    kill $WEBPID 2>/dev/null; wait $WEBPID 2>/dev/null || true
    (cd web && pnpm install --frozen-lockfile --silent) && start_web
  fi
  if changed; then
    touch "$STAMP"
    say "Rust code changed; rebuilding"
    if cargo build --quiet -p croncave-server -p croncave-agent; then
      [ "${COMPUTE_DRIVER:-local}" = "docker" ] && ./scripts/build-computer-image.sh >/dev/null
      kill $SERVER 2>/dev/null; wait $SERVER 2>/dev/null || true
      if start_server; then say "Restarted the control plane"; else warn "The control plane didn't start; see above"; fi
    else
      warn "The build failed; the previous version keeps running"
    fi
  elif ! kill -0 $SERVER 2>/dev/null; then
    warn "The control plane stopped; starting it again"
    sleep 1
    start_server || warn "It didn't start; fix the error above and save"
  fi
done
