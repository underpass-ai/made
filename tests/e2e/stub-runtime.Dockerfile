# syntax=docker/dockerfile:1.7
#
# Stub-runtime sidecar for the stack E2E.
#
# Builds the `choreo-stub-runtime` binary and ships it in a minimal,
# non-root image. Lets the choreographer's `RuntimeExecutor` talk to
# a real gRPC peer (`SessionService` + `InvocationService`) without
# dragging the real `underpass-runtime` image into this repo's test
# path. Driven by `tests/e2e/docker-compose.e2e.yaml`.

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

RUN --mount=type=cache,id=cargo-registry-stub-runtime,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-target-stub-runtime,target=/src/target \
    cargo build --release --locked --bin choreo-stub-runtime \
 && install -Dm 0755 target/release/choreo-stub-runtime /out/stub-runtime

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

LABEL org.opencontainers.image.title="underpass-choreographer-stub-runtime" \
      org.opencontainers.image.description="Canned-response stub of underpass.runtime.v1 for stack E2E. Not shipped." \
      org.opencontainers.image.vendor="Underpass AI" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.source="https://github.com/underpass-ai/underpass-choreographer"

COPY --from=builder /out/stub-runtime /usr/local/bin/choreo-stub-runtime

USER nonroot:nonroot

EXPOSE 50053

ENTRYPOINT ["/usr/local/bin/choreo-stub-runtime"]
