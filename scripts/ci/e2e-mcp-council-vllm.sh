#!/usr/bin/env bash
set -euo pipefail

# Runs the real-vLLM multi-agent council ceremony through the MCP stdio
# adapter. This is the MCP counterpart to `make e2e-council-vllm`,
# which drives the same ceremony through the gRPC E2E runner.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

export MADE_MCP_GRPC_ENDPOINT="${MADE_MCP_GRPC_ENDPOINT:-http://127.0.0.1:50055}"

if [[ -z "${MADE_MCP_BIN:-}" ]]; then
  export MADE_MCP_BIN="${ROOT_DIR}/target/debug/made-mcp"
  if [[ ! -x "${MADE_MCP_BIN}" ]]; then
    cargo build -p made-mcp --locked
  fi
fi

python3 "${ROOT_DIR}/scripts/mcp/made-mcp-council-vllm.py"
