#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${PLUGIN_ROOT}/bin/made-mcp"

if [[ ! -x "${BINARY}" && -x "${PLUGIN_ROOT}/bin/made-mcp.exe" ]]; then
  BINARY="${PLUGIN_ROOT}/bin/made-mcp.exe"
fi

if [[ ! -x "${BINARY}" ]]; then
  echo "MADE plugin: missing executable ${BINARY}" >&2
  echo "MADE plugin: build the local plugin bundle before installing it" >&2
  exit 127
fi

export MADE_MCP_BACKEND=embedded

# The embedded backend refuses to start without a state file: where
# ceremonies survive a restart is an operator decision. A plugin has no
# operator to ask, so it picks the conventional per-user state directory
# and says so — an explicit `MADE_MCP_REDB_PATH` always wins.
if [[ -z "${MADE_MCP_REDB_PATH:-}" ]]; then
  MADE_STATE_ROOT="${XDG_STATE_HOME:-${HOME}/.local/state}/underpass-made"
  mkdir -p "${MADE_STATE_ROOT}"
  export MADE_MCP_REDB_PATH="${MADE_STATE_ROOT}/ceremonies.redb"
fi

# Git Bash hands the native Windows binary an MSYS path it cannot open;
# cygpath converts it when, and only when, we are on such a host.
if command -v cygpath >/dev/null 2>&1; then
  MADE_MCP_REDB_PATH="$(cygpath -w "${MADE_MCP_REDB_PATH}")"
fi

exec "${BINARY}"
