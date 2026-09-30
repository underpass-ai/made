#!/usr/bin/env bash
set -euo pipefail

# MCP adapter against a real MADE server. testcontainers starts the
# published MADE image and the locally built `made-mcp` binary talks to it
# over gRPC, so the stdio <-> gRPC <-> server wiring is exercised end to
# end instead of against the in-process fixture.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
source "${ROOT_DIR}/scripts/ci/testcontainers-host.sh"

ensure_testcontainers_host

# The targets are discovered, not listed: every made-mcp test target gated
# on `container-tests` runs here (see integration-targets.sh).
suite_args="$(bash "${ROOT_DIR}/scripts/ci/integration-targets.sh" args image)"
mapfile -t suite <<<"${suite_args}"

# `cargo test` builds the package's own binary before its integration
# tests, which is the `made-mcp` the test spawns from the target directory.
RUST_TEST_THREADS=1 cargo test "${suite[@]}" --locked -- --test-threads=1
