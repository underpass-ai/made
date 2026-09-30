#!/usr/bin/env bash
set -euo pipefail

# Postgres integration tests. testcontainers spins a real Postgres per
# test so we validate wire behaviour — including the migration runner
# and JSONB roundtrip — instead of mocks.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
source "${ROOT_DIR}/scripts/ci/testcontainers-host.sh"

ensure_testcontainers_host

# Reuse the same acceptance suite when collecting instrumented coverage.
runner=(cargo test)
if [[ "${MADE_INTEGRATION_COVERAGE:-0}" == "1" ]]; then
  runner=(cargo llvm-cov --no-report)
fi

# The targets are discovered, not listed: every test target gated on
# `container-postgres` runs here (see integration-targets.sh).
suite_args="$(bash "${ROOT_DIR}/scripts/ci/integration-targets.sh" args postgres)"
mapfile -t suite <<<"${suite_args}"

# Keep container-backed suites single-threaded to avoid parallel startup
# spikes saturating the runner.
RUST_TEST_THREADS=1 "${runner[@]}" "${suite[@]}" --locked -- --test-threads=1
