#!/usr/bin/env bash
#
# Smoke test for the `made-mcp` stdio adapter.
#
# Fires one `tools/call` and asserts the response carries a JSON-RPC
# envelope plus an expected marker. Works in two modes:
#
#   - MADE_MCP_GRPC_ENDPOINT set: live mode. Hits the configured
#     MADE with `made_get_status` (a read-only RPC). Marker:
#     `"isError":false`.
#
#   - MADE_MCP_BACKEND=fixture: fixture mode. No network. Hits a
#     deterministic canned response for `made_list_councils`.
#     Marker: `"councils":[`.
#
# MADE_MCP_BIN (optional): override the binary name (default
# `made-mcp`). Useful when the binary is not yet on PATH and you want
# to point at a checkout build (`target/debug/made-mcp` etc.).
#
# Exit codes:
#   0 — smoke passed
#   1 — protocol or assertion failure
#   2 — invalid invocation (neither live nor fixture mode selected)

set -euo pipefail

MCP_BIN="${MADE_MCP_BIN:-made-mcp}"

if [[ -n "${MADE_MCP_GRPC_ENDPOINT:-}" ]]; then
  REQUEST='{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"made_get_status","arguments":{"include_stats":false}}}'
  EXPECTED='"isError":false'
else
  if [[ "${MADE_MCP_BACKEND:-}" != "fixture" ]]; then
    echo "MADE MCP smoke requires MADE_MCP_GRPC_ENDPOINT for live mode or MADE_MCP_BACKEND=fixture for fixture mode" >&2
    exit 2
  fi
  REQUEST='{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"made_list_councils","arguments":{}}}'
  EXPECTED='"councils":['
fi

RESPONSE="$(printf '%s\n' "${REQUEST}" | "${MCP_BIN}")"

printf '%s\n' "${RESPONSE}"

if ! grep -q '"jsonrpc":"2.0"' <<<"${RESPONSE}"; then
  echo "MADE MCP smoke failed: missing JSON-RPC envelope" >&2
  exit 1
fi

if grep -q '"isError":true' <<<"${RESPONSE}"; then
  echo "MADE MCP smoke failed: tool returned isError=true" >&2
  exit 1
fi

if ! grep -qF -- "${EXPECTED}" <<<"${RESPONSE}"; then
  echo "MADE MCP smoke failed: expected marker ${EXPECTED}" >&2
  exit 1
fi

echo "MADE MCP smoke passed" >&2
