#!/usr/bin/env bash
# Check everything: formatting, lints, unit and database tests, the web app, and the
# end-to-end tests in a real browser. CI runs this same script.
#
#   CHECK_SKIP_E2E=1 ./scripts/check.sh   # skip the browser tests
set -euo pipefail
. "$(dirname "$0")/lib.sh"
cd "$ROOT"
ensure_env

say "No color values outside the scheme data"
./scripts/check-tokens.sh

say "Rust: format"
cargo fmt --all --check

say "Rust: lints"
cargo clippy --workspace --all-targets --quiet -- -D warnings

say "Rust: tests (database tests use a real Postgres)"
DATABASE_URL="" ensure_postgres croncave_test
export TEST_DATABASE_URL="$DATABASE_URL"
# Each database test makes its own database; clear out the last run's.
for db in $(psql "$DATABASE_URL" -tAc "select datname from pg_database where datname like 'cc_test_%'"); do
  psql "$DATABASE_URL" -qc "drop database if exists $db with (force)" >/dev/null
done
cargo build --quiet -p croncave-agent
cargo test --workspace --quiet

if [ -d web ]; then
  say "Web: install"
  (cd web && pnpm install --frozen-lockfile --silent)
  say "Web: type check"
  (cd web && pnpm run check)
  say "Web: unit tests"
  (cd web && pnpm run test)
  say "Web: build"
  (cd web && pnpm run build >/dev/null)
fi

if [ -d e2e ] && [ "${CHECK_SKIP_E2E:-0}" != "1" ]; then
  say "End to end: Playwright"
  (cd e2e && pnpm install --frozen-lockfile --silent && pnpm test)
fi

say "All checks passed"
