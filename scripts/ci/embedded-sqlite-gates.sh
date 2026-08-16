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

echo "sqlite-gates: a store converts between engines without losing a record"
cargo test -p made-adapters --features sqlite --locked --test engine_conversion

echo "sqlite-gates: the adapter suite with the engine in (the redb arm must be unchanged)"
cargo test -p made-adapters --features sqlite --locked

# `cargo install` and a registry build resolve features without the workspace
# and without dev-dependencies. A feature that names a dev-dependency builds
# and tests green here and fails for everyone installing it — the exact defect
# kmp shipped and had to fix after the fact.
echo "sqlite-gates: the feature resolves outside the workspace"
cargo package -p made-adapters --features sqlite --locked --no-verify --allow-dirty >/dev/null

# The engine reaches an operator through `cargo install made-mcp --features
# sqlite`, so the feature has to forward all the way to the binary. Building
# the leaf crate proves nothing about the chain above it: kmp's sqlite feature
# was green at the adapter and still failed at install.
echo "sqlite-gates: the feature forwards to the binary operators install"
cargo build -p made-mcp --features sqlite --locked >/dev/null

# The conversion command is the only way an existing store reaches the engine
# that lets two hosts share it. A feature nobody can reach is not shipped, so
# this drives the real binary over a store with a ceremony in it: an empty
# conversion would pass while moving nothing.
echo "sqlite-gates: the conversion command moves a real store"
cargo build -p made-adapters --features sqlite --locked --bin store_writer >/dev/null
CONVERT_DIR="$(mktemp -d)"
trap 'rm -rf "${CONVERT_DIR}"' EXIT
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
"${TARGET_DIR}/debug/store_writer" "${CONVERT_DIR}/from.redb" redb convert-gate 2 >/dev/null
RECEIPT="$("${TARGET_DIR}/debug/made-mcp" convert \
  "${CONVERT_DIR}/from.redb" "${CONVERT_DIR}/to.sqlite3" --engine sqlite)"
echo "  ${RECEIPT}"
head -c 15 "${CONVERT_DIR}/to.sqlite3" | grep -q "SQLite format 3" \
  || { echo "sqlite-gates: convert did not write a sqlite store" >&2; exit 1; }
echo "${RECEIPT}" | grep -q '"ceremonies":1' \
  || { echo "sqlite-gates: convert moved no ceremony" >&2; exit 1; }

# Converting again into the same destination has to be refused: an operator
# who reruns the command must not silently half-merge two stores.
"${TARGET_DIR}/debug/made-mcp" convert \
  "${CONVERT_DIR}/from.redb" "${CONVERT_DIR}/to.sqlite3" --engine sqlite >/dev/null 2>&1 \
  && { echo "sqlite-gates: convert overwrote an occupied destination" >&2; exit 1; }
echo "  rerun into an occupied destination refused"

echo "sqlite-gates: passed"
