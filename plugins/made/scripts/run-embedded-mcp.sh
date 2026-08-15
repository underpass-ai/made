#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${PLUGIN_ROOT}/bin/made-mcp"

if [[ ! -x "${BINARY}" ]]; then
  echo "MADE plugin: missing executable ${BINARY}" >&2
  echo "MADE plugin: build the local plugin bundle before installing it" >&2
  exit 127
fi

export MADE_MCP_BACKEND=embedded
exec "${BINARY}"
