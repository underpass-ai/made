#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# An explicit binary wins over everything below.
BINARY="${MADE_MCP_BIN:-${PLUGIN_ROOT}/bin/made-mcp}"
if [[ -n "${MADE_MCP_BIN:-}" && ! -x "${MADE_MCP_BIN}" ]]; then
  echo "MADE plugin: MADE_MCP_BIN is set to '${MADE_MCP_BIN}', which is not executable." >&2
  exit 127
fi

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

# The binary and the plugin update through different commands — `cargo
# install --force` and `/plugin update` — and neither knows about the other.
# A stale plugin with a fresh binary keeps working by luck because this
# launcher falls back to PATH. Say it once, on stderr, where a host shows
# server output; never fail on it, since the mismatch is usually harmless.
PLUGIN_MANIFEST="${PLUGIN_ROOT}/.claude-plugin/plugin.json"
if [[ -f "${PLUGIN_MANIFEST}" ]] && command -v python3 >/dev/null 2>&1; then
  PLUGIN_VERSION="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["version"])' \
    "${PLUGIN_MANIFEST}" 2>/dev/null)"
  BINARY_VERSION="$("${BINARY}" --version 2>/dev/null | sed -E 's/^made-mcp ([^ ]+).*/\1/')"
  if [[ -n "${PLUGIN_VERSION}" && -n "${BINARY_VERSION}" && "${PLUGIN_VERSION}" != "${BINARY_VERSION}" ]]; then
    echo "MADE plugin: plugin files are ${PLUGIN_VERSION}, binary is ${BINARY_VERSION}." >&2
    echo "MADE plugin: they update separately — 'cargo install made-mcp --force' and" >&2
    echo "MADE plugin: '/plugin update made@underpass' — and fixes that live in this" >&2
    echo "MADE plugin: launcher or the skills come with the plugin, not the binary." >&2
  fi
fi

# The embedded backend refuses to start without a state file: where
# ceremonies survive a restart is an operator decision. A plugin has no
# operator to ask, so it picks the conventional per-user state directory
# and says so — an explicit `MADE_MCP_STORE_PATH` always wins.
if [[ -z "${MADE_MCP_STORE_PATH:-}" ]]; then
  USER_STATE_ROOT="${XDG_STATE_HOME:-${HOME}/.local/state}"
  MADE_STATE_ROOT="${USER_STATE_ROOT}/underpass-made"
  mkdir -p "${MADE_STATE_ROOT}"
  SQLITE_DEFAULT="${MADE_STATE_ROOT}/ceremonies.sqlite3"
  LEGACY_DEFAULT="${MADE_STATE_ROOT}/ceremonies.redb"
  if [[ ! -e "${SQLITE_DEFAULT}" && -e "${LEGACY_DEFAULT}" ]]; then
    echo "MADE plugin: legacy Redb store found at ${LEGACY_DEFAULT}." >&2
    echo "MADE plugin: convert it before upgrading with made-mcp v0.2.0:" >&2
    echo "MADE plugin:   made-mcp share-store '${LEGACY_DEFAULT}'" >&2
    echo "MADE plugin: the original is kept as a backup; no new store was created." >&2
    exit 2
  fi
  export MADE_MCP_STORE_PATH="${SQLITE_DEFAULT}"
fi

# Git Bash hands the native Windows binary an MSYS path it cannot open;
# cygpath converts it when, and only when, we are on such a host.
if command -v cygpath >/dev/null 2>&1; then
  MADE_MCP_STORE_PATH="$(cygpath -w "${MADE_MCP_STORE_PATH}")"
  export MADE_MCP_STORE_PATH
fi

exec "${BINARY}"
