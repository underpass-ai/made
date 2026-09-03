#!/usr/bin/env bash
# The canonical embedded SQLite store: contracts, concurrency and packaging.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo "sqlite-gates: lint the canonical adapter"
cargo clippy -p made-adapters --all-targets --features sqlite --locked -- -D warnings

echo "sqlite-gates: every persistence contract"
cargo test -p made-adapters --features sqlite --locked \
  --test sqlite_ceremony_store_conformance

echo "sqlite-gates: two processes, one store, nothing lost"
cargo test -p made-adapters --features sqlite --locked --test two_writers_one_store

echo "sqlite-gates: complete adapter suite"
cargo test -p made-adapters --features sqlite --locked

echo "sqlite-gates: public embedded facade"
cargo test -p made-embedded --locked --test sqlite_store_api

# The default install must carry SQLite; an engine-specific opt-in would make
# the documented embedded backend fail after installation.
echo "sqlite-gates: default cargo install carries SQLite"
INSTALL_ROOT="$(mktemp -d)"
trap 'rm -rf "${INSTALL_ROOT}"' EXIT
cargo install --path crates/made-mcp --locked --root "${INSTALL_ROOT}" --quiet
"${INSTALL_ROOT}/bin/made-mcp" --version | grep -q "embedded store: sqlite"

echo "sqlite-gates: passed"
