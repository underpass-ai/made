#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLUGIN_DIR="${ROOT_DIR}/plugins/made"

cd "${ROOT_DIR}"
TARGET_DIR="$(
  cargo metadata --no-deps --format-version 1 \
    | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])'
)"
if [[ -z "${TARGET_DIR}" ]]; then
  echo "MADE plugin build: cargo metadata returned an empty target directory" >&2
  exit 1
fi

cargo build --release --locked -p made-mcp --no-default-features --features embedded
mkdir -p "${PLUGIN_DIR}/bin"

BINARY="${TARGET_DIR}/release/made-mcp"
if [[ "${OS:-}" == "Windows_NT" ]]; then
  BINARY_CANDIDATES=("${BINARY}.exe" "${BINARY}")
else
  BINARY_CANDIDATES=("${BINARY}" "${BINARY}.exe")
fi

if [[ -f "${BINARY_CANDIDATES[0]}" ]]; then
  BUILT_BINARY="${BINARY_CANDIDATES[0]}"
elif [[ -f "${BINARY_CANDIDATES[1]}" ]]; then
  BUILT_BINARY="${BINARY_CANDIDATES[1]}"
else
  echo "MADE plugin build: built binary not found at ${BINARY} or ${BINARY}.exe" >&2
  exit 1
fi

if [[ "${BUILT_BINARY}" == *.exe ]]; then
  cp "${BUILT_BINARY}" "${PLUGIN_DIR}/bin/made-mcp.exe"
else
  cp "${BUILT_BINARY}" "${PLUGIN_DIR}/bin/made-mcp"
  chmod +x "${PLUGIN_DIR}/bin/made-mcp"
fi

echo "MADE plugin bundle ready at ${PLUGIN_DIR}"
