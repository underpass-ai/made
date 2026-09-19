# MADE — developer recipes.
#
# Every target here mirrors a CI gate (see scripts/ci/) so that
# `just <target>` produces the same result a PR check will produce.
# That keeps local iteration and CI on the same axis: when CI is
# red, `just` reproduces the failure on your machine bit-for-bit.
#
# List recipes: `just`.

# -----------------------------------------------------------------------------
# defaults
# -----------------------------------------------------------------------------

default:
    @just --list --unsorted

# Provider-feature matrix the linting/testing gates enable in CI.
# Mirrors .github/workflows/quality-gate.yml.
provider_features := "--features made-adapters/agent-anthropic --features made-adapters/agent-openai --features made-adapters/agent-vllm"

# -----------------------------------------------------------------------------
# development loop — match dev-loop.yml
# -----------------------------------------------------------------------------

# The draft-pull-request development loop: fmt + clippy + tests for the
# crates in DEV_PACKAGES, then the three architecture gates that cost
# seconds. This is not a second list that mirrors CI — it is the same
# script .github/workflows/dev-loop.yml runs, so the two cannot drift.
#
# Narrow or widen it for one run:
#   DEV_PACKAGES="-p made-mcp" just dev
#
# Run a single stage:
#   just dev lint | just dev test | just dev gates
#
# Nothing merges on this loop's word. `just check` is the gate.
dev STAGE='all':
    bash scripts/ci/dev-loop.sh {{STAGE}}

# Pin the draft/ready CI handover: the dev-loop triggers and draft guards,
# the quality-gate stand-down and its required `gate` context, and that
# `just dev` and the workflow name the same crates. Both workflows run it.
workflow-contract:
    python3 scripts/ci/dev-loop-workflow-contract.py --self-test
    python3 scripts/ci/quality-gate-plan.py --self-test
    bash scripts/ci/tree-already-proved.sh --self-test

# -----------------------------------------------------------------------------
# fast per-PR gates — match quality-gate.yml
# -----------------------------------------------------------------------------

# Contract gate: proto + AsyncAPI validation + blocking proto breaking check.
contract:
    bash scripts/ci/contract-gate.sh

# Format-check the whole workspace. Must pass before commit.
fmt-check:
    cargo fmt --all -- --check

# Apply formatting in place.
fmt:
    cargo fmt --all

# Assert that the embedded distribution does not pull remote infrastructure.
embedded-boundary:
    bash scripts/ci/embedded-dependency-boundary.sh

# Build and exercise the repo-local Codex plugin through its MCP launcher.
embedded-plugin-smoke:
    bash scripts/ci/made-plugin-smoke.sh

# Package the Codex / Claude Code plugin bundle into dist/plugin/.
plugin-package:
    bash scripts/plugin/package-made-plugin.sh

# Clippy on the full provider matrix, warnings-as-errors. Mirrors CI.
clippy:
    cargo clippy --workspace --all-targets --locked {{provider_features}} -- -D warnings

# Unit + in-process integration tests.
test:
    cargo test --workspace --locked {{provider_features}}

# Compile-check every bench (run them with `just bench-run`).
bench-compile:
    bash scripts/ci/bench-compile.sh

# Unit coverage with its 80 % floor — the same script the `coverage` job of
# quality-gate.yml runs on a ready pull request, when the impact planner
# routes it. `COVERAGE_MIN=n just coverage` moves the floor locally.
coverage:
    bash scripts/ci/rust-coverage.sh

# Walk the entire fast-gate cascade locally. Use before opening a PR.
check: workflow-contract contract fmt-check embedded-boundary embedded-plugin-smoke clippy test bench-compile

# -----------------------------------------------------------------------------
# container-backed checks — need Docker or Podman running
# -----------------------------------------------------------------------------

# NATS trigger + messaging round-trips against a real broker.
integration-nats:
    bash scripts/ci/integration-nats.sh

# Postgres adapter round-trips (deliberations, councils, agents,
# statistics) against a real Postgres.
integration-postgres:
    bash scripts/ci/integration-postgres.sh

# Every container-backed integration test.
integration: integration-nats integration-postgres

# -----------------------------------------------------------------------------
# chart & image
# -----------------------------------------------------------------------------

# Helm lint + every hardened-render assertion.
helm-lint:
    bash scripts/ci/helm-lint.sh

# Build the production container image through the CI script so
# the dockerfile + entrypoint match what CI/CD ships.
build-image:
    bash scripts/ci/container-image.sh

# Build the provider-E2E runner image. Not published — operators
# push to whatever registry their cluster can pull from. Tag via
# IMAGE_TAG=<tag>.
build-provider-image:
    bash scripts/ci/build-provider-image.sh

# -----------------------------------------------------------------------------
# running the binary
# -----------------------------------------------------------------------------

# Run the binary locally. Wrapper for development — config via env.
run *ARGS='':
    cargo run --locked -p made {{ARGS}}

# Run with the `otel` feature enabled so the OTLP exporter is
# available when MADE_OTLP_ENDPOINT is set.
run-otel *ARGS='':
    cargo run --locked -p made --features otel {{ARGS}}

# -----------------------------------------------------------------------------
# benches (manual — CI only compile-checks them)
# -----------------------------------------------------------------------------

# Run the TraceContext micro-benches. Numbers land in
# docs/experiments/001-baseline-deliberation-latency/results/.
bench-trace:
    cargo bench -p made-core --bench trace_context

# Run the DeliberateUseCase end-to-end bench.
bench-deliberate:
    cargo bench -p made-app --bench deliberate

# Reproduce experiment 001.
bench-experiment-001:
    bash docs/experiments/001-baseline-deliberation-latency/run.sh

# Reproduce experiment 002.
bench-experiment-002:
    bash docs/experiments/002-deliberation-scale-sweep/run.sh

# -----------------------------------------------------------------------------
# release
# -----------------------------------------------------------------------------

# Bump every versioned artefact in one place. Takes a semver
# argument: `just version 0.2.0`.
version VERSION:
    bash scripts/release.sh version {{VERSION}}

# Cut a stable or prerelease: tag HEAD with `v{VERSION}` and push. Requires the
# working tree clean and every version in sync.
release VERSION:
    bash scripts/release.sh release {{VERSION}}
