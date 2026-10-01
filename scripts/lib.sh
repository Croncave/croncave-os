# Shared helpers for scripts/dev.sh and scripts/check.sh. Source it; don't run it.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEV_DIR="${CRONCAVE_DEV_DIR:-$ROOT/.dev}"
PG_PORT="${CRONCAVE_PG_PORT:-54329}"

say() { printf '\033[1;32m›\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31m✗\033[0m %s\n' "$*" >&2; exit 1; }

# Create .env from .env.example, generating the secrets, if it doesn't exist.
ensure_env() {
  if [ ! -f "$ROOT/.env" ]; then
    say "Creating .env from .env.example"
    local secrets_key relay_secret
    secrets_key="$(od -An -tx1 -N32 /dev/urandom | tr -d ' \n')"
    relay_secret="$(od -An -tx1 -N24 /dev/urandom | tr -d ' \n')"
    sed -e "s/^SECRETS_KEY=$/SECRETS_KEY=$secrets_key/" -e "s/^RELAY_SECRET=$/RELAY_SECRET=$relay_secret/" \
      "$ROOT/.env.example" > "$ROOT/.env"
  fi
  set -a
  # shellcheck disable=SC1091
  . "$ROOT/.env"
  set +a
}

find_pg_bin() {
  if command -v pg_ctl >/dev/null 2>&1; then dirname "$(command -v pg_ctl)"; return; fi
  for d in /usr/lib/postgresql/*/bin /opt/homebrew/opt/postgresql@*/bin /usr/local/opt/postgresql@*/bin \
           /opt/homebrew/bin /usr/local/bin /Applications/Postgres.app/Contents/Versions/latest/bin; do
    [ -x "$d/pg_ctl" ] && { echo "$d"; return; }
  done
}

# Postgres refuses to run as root; in containers run it as the postgres user.
as_pg() {
  if [ "$(id -u)" = "0" ]; then
    runuser -u postgres -- "$@"
  else
    "$@"
  fi
}

# Make sure a Postgres is reachable and export DATABASE_URL.
#   1. DATABASE_URL already set and reachable: use it.
#   2. Postgres binaries installed: a private cluster in .dev/postgres.
#   3. Docker running: a postgres:16 container.
ensure_postgres() {
  local db="${1:-croncave}"
  if [ -n "${DATABASE_URL:-}" ] && psql "$DATABASE_URL" -c 'select 1' >/dev/null 2>&1; then
    return
  fi
  local url="postgres://croncave@127.0.0.1:$PG_PORT/$db" pg_bin
  # Homebrew's versioned Postgres isn't on PATH; its psql and pg_isready are needed too.
  pg_bin="$(find_pg_bin)"
  [ -n "$pg_bin" ] && PATH="$pg_bin:$PATH"
  if ! pg_isready -h 127.0.0.1 -p "$PG_PORT" >/dev/null 2>&1; then
    local bin
    bin="$(find_pg_bin)"
    if [ -n "$bin" ]; then
      local data="$DEV_DIR/postgres"
      mkdir -p "$DEV_DIR"
      if [ ! -f "$data/PG_VERSION" ]; then
        say "Creating a local Postgres in $data"
        mkdir -p "$data"
        [ "$(id -u)" = "0" ] && chown -R postgres "$data"
        as_pg "$bin/initdb" -D "$data" -U croncave --auth=trust -E UTF8 >/dev/null
      fi
      mkdir -p "$DEV_DIR/pgsock"
      [ "$(id -u)" = "0" ] && chown postgres "$DEV_DIR/pgsock" "$DEV_DIR"
      say "Starting Postgres on port $PG_PORT"
      as_pg "$bin/pg_ctl" -D "$data" -l "$data/server.log" -w \
        -o "-p $PG_PORT -c listen_addresses=127.0.0.1 -k $DEV_DIR/pgsock -c max_connections=200 -c fsync=off" start >/dev/null \
        || die "Postgres didn't start; see $data/server.log"
    elif command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
      say "Starting Postgres in Docker on port $PG_PORT"
      docker run -d --rm --name croncave-postgres -p "127.0.0.1:$PG_PORT:5432" \
        -e POSTGRES_USER=croncave -e POSTGRES_HOST_AUTH_METHOD=trust postgres:16 >/dev/null
      for _ in $(seq 1 60); do pg_isready -h 127.0.0.1 -p "$PG_PORT" >/dev/null 2>&1 && break; sleep 0.5; done
    else
      die "No Postgres found. Install Postgres 16 (e.g. brew install postgresql@16), start Docker, or set DATABASE_URL."
    fi
  fi
  psql "postgres://croncave@127.0.0.1:$PG_PORT/postgres" -tAc "select 1 from pg_database where datname = '$db'" | grep -q 1 \
    || psql "postgres://croncave@127.0.0.1:$PG_PORT/postgres" -qc "create database $db" >/dev/null
  export DATABASE_URL="$url"
}

stop_postgres() {
  local bin data="$DEV_DIR/postgres"
  bin="$(find_pg_bin)"
  if [ -n "$bin" ] && [ -f "$data/postmaster.pid" ]; then
    as_pg "$bin/pg_ctl" -D "$data" -m fast stop >/dev/null 2>&1 || true
  fi
  docker rm -f croncave-postgres >/dev/null 2>&1 || true
}
