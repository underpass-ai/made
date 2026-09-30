#!/usr/bin/env bash
set -euo pipefail

# NATS integration tests. Backed by testcontainers — the test harness
# spins up a real NATS server in a container for each run so we validate
# the real wire behaviour instead of mocks.
#
# Only the NATS-focused tests run here. Postgres-backed tests belong to
# `integration-postgres.sh` so each CI job covers exactly one transport
# boundary.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
source "${ROOT_DIR}/scripts/ci/testcontainers-host.sh"

ensure_testcontainers_host

# The targets are discovered, not listed: every test target gated on
# `container-nats` runs here (see integration-targets.sh).
suite_args="$(bash "${ROOT_DIR}/scripts/ci/integration-targets.sh" args nats)"
mapfile -t suite <<<"${suite_args}"

# Keep container-backed suites single-threaded to avoid parallel startup
# spikes saturating the runner.
RUST_TEST_THREADS=1 cargo test "${suite[@]}" --locked -- --test-threads=1
