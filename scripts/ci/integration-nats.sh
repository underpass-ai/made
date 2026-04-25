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

TEST_CRATE="crates/choreo-tests-integration/Cargo.toml"
NATS_TEST_A="crates/choreo-tests-integration/tests/nats_messaging_roundtrip.rs"
NATS_TEST_B="crates/choreo-tests-integration/tests/nats_trigger_subscriber.rs"

if [ ! -f "${TEST_CRATE}" ] || [ ! -f "${NATS_TEST_A}" ] || [ ! -f "${NATS_TEST_B}" ]; then
  echo "::warning::NATS integration suite not present; skipping"
  exit 0
fi

ensure_testcontainers_host

# Keep container-backed suites single-threaded to avoid parallel startup
# spikes saturating the runner.
RUST_TEST_THREADS=1 cargo test \
  -p choreo-tests-integration \
  --features container-tests \
  --test nats_messaging_roundtrip \
  --test nats_trigger_subscriber \
  --locked \
  -- --test-threads=1
