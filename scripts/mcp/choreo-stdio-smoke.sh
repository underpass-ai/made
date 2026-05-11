#!/usr/bin/env bash
#
# Smoke test for the `choreo-mcp` stdio adapter.
#
# Fires one `tools/call` and asserts the response carries a JSON-RPC
# envelope plus an expected marker. Works in two modes:
#
#   - CHOREO_MCP_GRPC_ENDPOINT set: live mode. Hits the configured
#     choreographer with `choreo_get_status` (a read-only RPC). Marker:
#     `"isError":false`.
#
#   - CHOREO_MCP_BACKEND=fixture: fixture mode. No network. Hits a
#     deterministic canned response for `choreo_list_councils`.
#     Marker: `"councils":[`.
#
# CHOREO_MCP_BIN (optional): override the binary name (default
# `choreo-mcp`). Useful when the binary is not yet on PATH and you want
# to point at a checkout build (`target/debug/choreo-mcp` etc.).
#
# Exit codes:
#   0 — smoke passed
#   1 — protocol or assertion failure
#   2 — invalid invocation (neither live nor fixture mode selected)

set -euo pipefail

MCP_BIN="${CHOREO_MCP_BIN:-choreo-mcp}"

if [[ -n "${CHOREO_MCP_GRPC_ENDPOINT:-}" ]]; then
  REQUEST='{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"choreo_get_status","arguments":{"include_stats":false}}}'
  EXPECTED='"isError":false'
else
  if [[ "${CHOREO_MCP_BACKEND:-}" != "fixture" ]]; then
    echo "choreo MCP smoke requires CHOREO_MCP_GRPC_ENDPOINT for live mode or CHOREO_MCP_BACKEND=fixture for fixture mode" >&2
    exit 2
  fi
  REQUEST='{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"choreo_list_councils","arguments":{}}}'
  EXPECTED='"councils":['
fi

RESPONSE="$(printf '%s\n' "${REQUEST}" | "${MCP_BIN}")"

printf '%s\n' "${RESPONSE}"

if ! grep -q '"jsonrpc":"2.0"' <<<"${RESPONSE}"; then
  echo "choreo MCP smoke failed: missing JSON-RPC envelope" >&2
  exit 1
fi

if grep -q '"isError":true' <<<"${RESPONSE}"; then
  echo "choreo MCP smoke failed: tool returned isError=true" >&2
  exit 1
fi

if ! grep -qF -- "${EXPECTED}" <<<"${RESPONSE}"; then
  echo "choreo MCP smoke failed: expected marker ${EXPECTED}" >&2
  exit 1
fi

echo "choreo MCP smoke passed" >&2
