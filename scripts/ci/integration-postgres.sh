#!/usr/bin/env bash
set -euo pipefail

# Postgres integration tests. testcontainers spins a real Postgres per
# test so we validate wire behaviour — including the migration runner
# and JSONB roundtrip — instead of mocks.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
source "${ROOT_DIR}/scripts/ci/testcontainers-host.sh"

ensure_testcontainers_host

# Keep container-backed suites single-threaded to avoid parallel startup
# spikes saturating the runner.
RUST_TEST_THREADS=1 cargo test \
  -p made-tests-integration \
  --features container-tests \
  --test postgres_deliberation_repository \
  --test postgres_council_registry \
  --test postgres_agent_registry \
  --test postgres_statistics \
  --test postgres_ceremony_store \
  --test postgres_budget_snapshot \
  --test postgres_ceremony_ha \
  --test postgres_children_recovery \
  --test council_journal_conformance \
  --test council_snapshot_migration \
  --test artifact_postgres_store \
  --test authorization_postgres_store \
  --locked \
  -- --test-threads=1
