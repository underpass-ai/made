# syntax=docker/dockerfile:1.7
#
# E2E runner container.
#
# Builds the `made-e2e-runner` binary and ships it in a minimal,
# non-root image. Driven by the compose stack under
# `tests/e2e/docker-compose.e2e.yaml`.

ARG RUST_VERSION=1.90.0
ARG DEBIAN_RELEASE=bookworm

FROM docker.io/library/rust:${RUST_VERSION}-${DEBIAN_RELEASE} AS builder

ENV CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=always \
    RUSTFLAGS="-C strip=symbols"

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      protobuf-compiler \
      libprotobuf-dev \
      ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./
COPY crates ./crates
COPY tests/e2e/ceremonies ./tests/e2e/ceremonies
# Ship the canonical Report JSON Schema alongside the runner binary
# so scenario 8 can read it inside the container. Pinned to a stable
# path; compose sets `MADE_REPORT_SCHEMA_PATH=/etc/made/report.schema.json`
# so the runner discovers it without baking the path into the binary.
COPY api/examples/output-contracts/report.schema.json /etc/made/report.schema.json

RUN --mount=type=cache,id=cargo-registry-e2e-runner,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-target-e2e-runner,target=/src/target \
    cargo build --release --locked --bin made-e2e-runner \
 && install -Dm 0755 target/release/made-e2e-runner /out/runner

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

LABEL org.opencontainers.image.title="made-e2e-runner" \
      org.opencontainers.image.description="Drives the MADE over gRPC for E2E tests. Not shipped." \
      org.opencontainers.image.vendor="Underpass AI" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.source="https://github.com/underpass-ai/made"

COPY --from=builder /out/runner /usr/local/bin/made-e2e-runner
COPY --from=builder /etc/made/report.schema.json /etc/made/report.schema.json

USER nonroot:nonroot

ENTRYPOINT ["/usr/local/bin/made-e2e-runner"]
