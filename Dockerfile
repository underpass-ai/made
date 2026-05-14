# syntax=docker/dockerfile:1.7
#
# Underpass Choreographer — multi-stage build.
# Works identically under docker and podman. Produces a minimal
# distroless-style runtime image that runs as a non-root user.

ARG RUST_VERSION=1.90.0
ARG DEBIAN_RELEASE=bookworm

# ---------------------------------------------------------------------------
# Builder
# ---------------------------------------------------------------------------
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

# `agent-openai` is enabled so the `DispatchingAgentFactory` can
# materialise `kind=openai` agents that the compose E2E registers
# dynamically against the stub-llm sidecar. Without this feature flag
# the factory rejects `openai` with "unsupported agent kind". Other
# provider features stay disabled here to keep the production image
# minimal — operators that want anthropic or vllm must build a
# downstream image that flips the corresponding flag.
RUN --mount=type=cache,id=cargo-registry-choreo,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-target-choreo,target=/src/target \
    cargo build --release --locked --bin choreo --features choreo-adapters/agent-openai \
 && install -Dm 0755 target/release/choreo /out/choreo

# ---------------------------------------------------------------------------
# Runtime
# ---------------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

LABEL org.opencontainers.image.title="underpass-choreographer" \
      org.opencontainers.image.description="Event-driven coordinator of specialist agent councils. Use-case agnostic." \
      org.opencontainers.image.vendor="Underpass AI" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.source="https://github.com/underpass-ai/underpass-choreographer"

COPY --from=builder /out/choreo /usr/local/bin/choreo

USER nonroot:nonroot

EXPOSE 50055

ENTRYPOINT ["/usr/local/bin/choreo"]
