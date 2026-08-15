#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLUGIN_DIR="${ROOT_DIR}/plugins/made"
BINARY="${ROOT_DIR}/target/release/made-mcp"

cd "${ROOT_DIR}"
cargo build --release --locked -p made-mcp --no-default-features --features embedded
mkdir -p "${PLUGIN_DIR}/bin"
if [[ -f "${BINARY}" ]]; then
  cp "${BINARY}" "${PLUGIN_DIR}/bin/made-mcp"
  chmod +x "${PLUGIN_DIR}/bin/made-mcp"
fi
if [[ -f "${BINARY}.exe" ]]; then
  cp "${BINARY}.exe" "${PLUGIN_DIR}/bin/made-mcp.exe"
fi

echo "MADE plugin bundle ready at ${PLUGIN_DIR}"
