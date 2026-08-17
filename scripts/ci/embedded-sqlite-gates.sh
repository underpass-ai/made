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

# `cargo install` resolves features without dev-dependencies. A feature that
# names one builds and tests green here and fails for everyone installing it —
# the exact defect kmp shipped and had to fix after the fact. This runs the
# command a user actually runs, which is also the only form that survives a
# version bump: `cargo package` would resolve the sibling crates against the
# registry, where the version being released does not exist yet.
echo "sqlite-gates: cargo install with the engine, the way a user gets it"
INSTALL_ROOT="$(mktemp -d)"
CONVERT_DIR="$(mktemp -d)"
trap 'rm -rf "${INSTALL_ROOT}" "${CONVERT_DIR}"' EXIT
cargo install --path crates/made-mcp --features sqlite --locked --root "${INSTALL_ROOT}" --quiet
"${INSTALL_ROOT}/bin/made-mcp" --version

# The conversion command is the only way an existing store reaches the engine
# that lets two hosts share it. A feature nobody can reach is not shipped, so
# this drives the real binary over a store with a ceremony in it: an empty
# conversion would pass while moving nothing.
echo "sqlite-gates: the conversion command moves a real store"
cargo build -p made-adapters --features sqlite --locked --bin store_writer >/dev/null
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
"${TARGET_DIR}/debug/store_writer" "${CONVERT_DIR}/from.redb" redb convert-gate 2 >/dev/null
RECEIPT="$("${INSTALL_ROOT}/bin/made-mcp" convert \
  "${CONVERT_DIR}/from.redb" "${CONVERT_DIR}/to.sqlite3" --engine sqlite)"
echo "  ${RECEIPT}"
head -c 15 "${CONVERT_DIR}/to.sqlite3" | grep -q "SQLite format 3" \
  || { echo "sqlite-gates: convert did not write a sqlite store" >&2; exit 1; }
echo "${RECEIPT}" | grep -q '"ceremonies":1' \
  || { echo "sqlite-gates: convert moved no ceremony" >&2; exit 1; }

# Converting again into the same destination has to be refused: an operator
# who reruns the command must not silently half-merge two stores.
"${INSTALL_ROOT}/bin/made-mcp" convert \
  "${CONVERT_DIR}/from.redb" "${CONVERT_DIR}/to.sqlite3" --engine sqlite >/dev/null 2>&1 \
  && { echo "sqlite-gates: convert overwrote an occupied destination" >&2; exit 1; }
echo "  rerun into an occupied destination refused"

# `share-store` is the command that exists so nobody repeats the manual
# sequence. It earns its place only if it verifies, refuses and keeps the
# original, so drive all three rather than the happy path alone.
echo "sqlite-gates: share-store converts, verifies, names by engine and keeps the original"
SHARE_DIR="$(mktemp -d)"
trap 'rm -rf "${INSTALL_ROOT}" "${CONVERT_DIR}" "${SHARE_DIR}"' EXIT
"${TARGET_DIR}/debug/store_writer" "${SHARE_DIR}/ceremonies.redb" redb share-gate 3 >/dev/null

OUTPUT="$("${INSTALL_ROOT}/bin/made-mcp" share-store "${SHARE_DIR}/ceremonies.redb")"
echo "${OUTPUT}" | sed 's/^/    /'
echo "${OUTPUT}" | grep -q "verified:" \
  || { echo "sqlite-gates: share-store installed without verifying" >&2; exit 1; }
head -c 15 "${SHARE_DIR}/ceremonies.sqlite3" | grep -q "SQLite format 3" \
  || { echo "sqlite-gates: the converted store is not named by its engine" >&2; exit 1; }
[ -f "${SHARE_DIR}/ceremonies.redb.redb-before-share" ] \
  || { echo "sqlite-gates: share-store did not keep the original" >&2; exit 1; }
[ -f "${SHARE_DIR}/ceremonies.redb" ] \
  && { echo "sqlite-gates: two live stores were left behind" >&2; exit 1; }

"${INSTALL_ROOT}/bin/made-mcp" share-store "${SHARE_DIR}/ceremonies.sqlite3" \
  | grep -q "already shareable" \
  || { echo "sqlite-gates: share-store is not idempotent" >&2; exit 1; }
echo "    rerun: already shareable"

echo "sqlite-gates: passed"
