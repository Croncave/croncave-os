#!/usr/bin/env bash
# Colors come from semantic tokens only: no hex or rgb() values in product code.
# Schemes are data (crates/server/catalog/scheme-*.json); everything else uses tokens.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -d web/src ] || exit 0
hits=$(grep -rnE '#[0-9a-fA-F]{3,8}\b|rgba?\([[:space:]]*[0-9]|hsla?\([[:space:]]*[0-9]' web/src \
  --include='*.svelte' --include='*.ts' --include='*.css' --include='*.html' \
  | grep -vE '\.test\.ts:' \
  | grep -vE '&#[0-9]+;' || true)
if [ -n "$hits" ]; then
  echo "Color values found outside the scheme data; use a token (var(--...)) instead:" >&2
  echo "$hits" >&2
  exit 1
fi
