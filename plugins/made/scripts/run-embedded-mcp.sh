#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${PLUGIN_ROOT}/bin/made-mcp"

if [[ ! -x "${BINARY}" && -x "${PLUGIN_ROOT}/bin/made-mcp.exe" ]]; then
  BINARY="${PLUGIN_ROOT}/bin/made-mcp.exe"
fi

# The release bundle ships bin/made-mcp and keeps priority: it pins the binary
# this plugin version was tested against. An install straight from the
# repository has no bin/ — that path is gitignored — so fall back to an
# installed made-mcp on PATH rather than leaving the host with a server that
# cannot start.
if [[ ! -x "${BINARY}" ]]; then
  if PATH_BINARY="$(command -v made-mcp 2>/dev/null)"; then
    BINARY="${PATH_BINARY}"
  fi
fi

if [[ ! -x "${BINARY}" ]]; then
  echo "MADE plugin: no made-mcp executable found." >&2
  echo "MADE plugin: looked for ${PLUGIN_ROOT}/bin/made-mcp (release bundle) and made-mcp on PATH." >&2
  echo "MADE plugin: install one with 'cargo install made-mcp', or install the plugin from a release package." >&2
  exit 127
fi

export MADE_MCP_BACKEND=embedded

# The embedded backend refuses to start without a state file: where
# ceremonies survive a restart is an operator decision. A plugin has no
# operator to ask, so it picks the conventional per-user state directory
# and says so — an explicit `MADE_MCP_REDB_PATH` always wins.
if [[ -z "${MADE_MCP_REDB_PATH:-}" ]]; then
  USER_STATE_ROOT="${XDG_STATE_HOME:-${HOME}/.local/state}"
  MADE_STATE_ROOT="${USER_STATE_ROOT}/underpass-made"
  mkdir -p "${MADE_STATE_ROOT}"
  export MADE_MCP_REDB_PATH="${MADE_STATE_ROOT}/ceremonies.redb"

  # A store converted to the sqlite engine lives beside the redb one under a
  # name of its own, and the whole point of converting is that both agent
  # hosts open it. Making the operator also hand every host an explicit path
  # would put the shared store out of reach of the install we document. So:
  # asking for sqlite picks the sqlite name, and a converted store already
  # sitting there is used without being asked for. If both files exist the
  # default stays redb — that ambiguity is the operator's to resolve, not
  # something to guess behind their back.
  SQLITE_DEFAULT="${MADE_STATE_ROOT}/ceremonies.sqlite3"
  if [[ "${MADE_MCP_ENGINE:-}" == "sqlite" ]]; then
    export MADE_MCP_REDB_PATH="${SQLITE_DEFAULT}"
  elif [[ -f "${SQLITE_DEFAULT}" && ! -e "${MADE_MCP_REDB_PATH}" ]]; then
    export MADE_MCP_REDB_PATH="${SQLITE_DEFAULT}"
  fi

  # First start after the rename imports the former default automatically.
  # The legacy file remains read-only evidence; MADE writes a separate file.
  LEGACY_DEFAULT="${USER_STATE_ROOT}/underpass-choreographer/ceremonies.redb"
  if [[ ! -e "${MADE_MCP_REDB_PATH}" && -f "${LEGACY_DEFAULT}" && -z "${MADE_MCP_LEGACY_REDB_PATH:-}" ]]; then
    export MADE_MCP_LEGACY_REDB_PATH="${LEGACY_DEFAULT}"
  fi
fi

# Git Bash hands the native Windows binary an MSYS path it cannot open;
# cygpath converts it when, and only when, we are on such a host.
if command -v cygpath >/dev/null 2>&1; then
  MADE_MCP_REDB_PATH="$(cygpath -w "${MADE_MCP_REDB_PATH}")"
  if [[ -n "${MADE_MCP_LEGACY_REDB_PATH:-}" ]]; then
    MADE_MCP_LEGACY_REDB_PATH="$(cygpath -w "${MADE_MCP_LEGACY_REDB_PATH}")"
  fi
fi

exec "${BINARY}"
