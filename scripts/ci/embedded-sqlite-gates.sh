#!/usr/bin/env bash
# The SQLite engine behind the storage seam.
#
# Everything the default build proves about the embedded store, proved again
# with the opt-in engine compiled in — plus the one thing only this engine
# claims: two processes can hold one store without losing a record. The
# default build's own gates stay untouched; this runs alongside them, never
# instead of them.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo "sqlite-gates: clippy with the engine compiled in"
cargo clippy -p made-adapters --all-targets --features sqlite --locked -- -D warnings

echo "sqlite-gates: every store contract on the sqlite engine"
cargo test -p made-adapters --features sqlite --locked \
  --test sqlite_ceremony_store_conformance

echo "sqlite-gates: two processes, one store, nothing lost"
cargo test -p made-adapters --features sqlite --locked --test two_writers_one_store

echo "sqlite-gates: the adapter suite with the engine in (the redb arm must be unchanged)"
cargo test -p made-adapters --features sqlite --locked

# `cargo install` and a registry build resolve features without the workspace
# and without dev-dependencies. A feature that names a dev-dependency builds
# and tests green here and fails for everyone installing it — the exact defect
# kmp shipped and had to fix after the fact.
echo "sqlite-gates: the feature resolves outside the workspace"
cargo package -p made-adapters --features sqlite --locked --no-verify --allow-dirty >/dev/null

echo "sqlite-gates: passed"
