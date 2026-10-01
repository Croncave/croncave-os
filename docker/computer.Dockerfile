# The image every Docker-run computer boots: a plain Linux with the runtimes the Scripts
# app offers (Python, Node.js, shell) and the Croncave agent as its only service.
#
#   docker build -f docker/computer.Dockerfile -t croncave-computer .            # builds the agent here
#   docker build -f docker/computer.Dockerfile --target prebuilt -t croncave-computer <dir with croncave-agent>
#
# scripts/build-computer-image.sh picks the right one.

FROM rust:1-slim-trixie AS builder
RUN apt-get update && apt-get install -y --no-install-recommends cmake clang pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates crates
# Behind a TLS-inspecting proxy, pass its CA bundle as the build secret "ca_bundle".
RUN --mount=type=secret,id=ca_bundle,required=false \
    if [ -f /run/secrets/ca_bundle ]; then export CARGO_HTTP_CAINFO=/run/secrets/ca_bundle SSL_CERT_FILE=/run/secrets/ca_bundle; fi; \
    cargo build --release -p croncave-agent && cp target/release/croncave-agent /croncave-agent

FROM debian:trixie-slim AS base
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates python3 python3-pip nodejs npm bash procps coreutils tar \
 && rm -rf /var/lib/apt/lists/*
# The person's files and the apps' own data live on the mounted disk.
VOLUME /croncave/disk
ENV CRONCAVE_DISK=/croncave/disk LANG=C.UTF-8
STOPSIGNAL SIGTERM

# An agent built on a Linux host (glibc 2.39 or older), copied in.
FROM base AS prebuilt
COPY croncave-agent /usr/local/bin/croncave-agent
ENTRYPOINT ["/usr/local/bin/croncave-agent"]

# The default: the agent built above, so this works from macOS too.
FROM base AS computer
COPY --from=builder /croncave-agent /usr/local/bin/croncave-agent
ENTRYPOINT ["/usr/local/bin/croncave-agent"]
