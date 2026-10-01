#!/usr/bin/env bash
# Build the image Docker-run computers boot from (COMPUTE_DRIVER=docker).
# On Linux it copies the agent built here (fast); elsewhere it builds the agent in Docker.
set -euo pipefail
. "$(dirname "$0")/lib.sh"
cd "$ROOT"
IMAGE="${COMPUTER_IMAGE:-croncave-computer:dev}"
docker info >/dev/null 2>&1 || die "Docker isn't running. Start Docker, or use COMPUTE_DRIVER=local."

if [ "$(uname -s)" = "Linux" ] && [ "${BUILD_AGENT_IN_DOCKER:-0}" != "1" ]; then
  cargo build --quiet -p croncave-agent
  ctx="$(mktemp -d)"
  trap 'rm -rf "$ctx"' EXIT
  cp target/debug/croncave-agent "$ctx/"
  say "Building $IMAGE with this machine's agent"
  docker build --quiet -f docker/computer.Dockerfile --target prebuilt -t "$IMAGE" "$ctx" >/dev/null
else
  say "Building $IMAGE (compiles the agent in Docker; the first build takes a few minutes)"
  secret=()
  # Behind a TLS-inspecting proxy, the build needs its CA bundle (SSL_CERT_FILE).
  [ -n "${SSL_CERT_FILE:-}" ] && [ -f "$SSL_CERT_FILE" ] && secret=(--secret "id=ca_bundle,src=$SSL_CERT_FILE")
  docker build --quiet "${secret[@]}" -f docker/computer.Dockerfile --target computer -t "$IMAGE" . >/dev/null
fi
say "Built $IMAGE"
