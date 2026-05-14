# syntax=docker/dockerfile:1.7
#
# Stub-LLM sidecar for the stack E2E.
#
# Builds the `choreo-stub-llm` binary and ships it in a minimal,
# non-root image. The container serves the OpenAI Chat Completions
# shape on `:8000` and always returns a JSON Report payload that
# satisfies `api/examples/output-contracts/report.schema.json`.
# That lets the compose E2E exercise the positive structured-output
# path through the choreographer's `OpenAiAgent` adapter without a
# real provider in the test path. Driven by
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

RUN --mount=type=cache,id=cargo-registry-stub-llm,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-target-stub-llm,target=/src/target \
    cargo build --release --locked --bin choreo-stub-llm \
 && install -Dm 0755 target/release/choreo-stub-llm /out/stub-llm

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

LABEL org.opencontainers.image.title="underpass-choreographer-stub-llm" \
      org.opencontainers.image.description="OpenAI-shaped stub LLM that always returns a Report-shaped JSON payload. Not shipped." \
      org.opencontainers.image.vendor="Underpass AI" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.source="https://github.com/underpass-ai/underpass-choreographer"

COPY --from=builder /out/stub-llm /usr/local/bin/choreo-stub-llm

USER nonroot:nonroot

EXPOSE 8000

ENTRYPOINT ["/usr/local/bin/choreo-stub-llm"]
