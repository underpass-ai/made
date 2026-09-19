#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=made-embedded-config.sh
source "${PLUGIN_ROOT}/scripts/made-embedded-config.sh"

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
  echo "MADE plugin: run /made:setup in Claude Code or the made-setup skill in Codex." >&2
  exit 127
fi

export MADE_MCP_BACKEND=embedded

# A PATH fallback can differ from the plugin version. Say it once, on stderr,
# where a host shows server output; setup restores the release-matched binary.
PLUGIN_MANIFEST="${PLUGIN_ROOT}/.claude-plugin/plugin.json"
if [[ -f "${PLUGIN_MANIFEST}" ]] && command -v python3 >/dev/null 2>&1; then
  PLUGIN_VERSION="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["version"])' \
    "${PLUGIN_MANIFEST}" 2>/dev/null)"
  BINARY_VERSION="$("${BINARY}" --version 2>/dev/null | sed -E 's/^made-mcp ([^ ]+).*/\1/')"
  if [[ -n "${PLUGIN_VERSION}" && -n "${BINARY_VERSION}" && "${PLUGIN_VERSION}" != "${BINARY_VERSION}" ]]; then
    echo "MADE plugin: plugin files are ${PLUGIN_VERSION}, binary is ${BINARY_VERSION}." >&2
    echo "MADE plugin: update the plugin, then run /made:setup in Claude Code" >&2
    echo "MADE plugin: or the made-setup skill in Codex to restore release parity." >&2
  fi
fi

MADE_MCP_STORE_PATH="$(made_embedded_store_path)"
export MADE_MCP_STORE_PATH

# Git Bash hands the native Windows binary an MSYS path it cannot open;
# cygpath converts it when, and only when, we are on such a host.
if command -v cygpath >/dev/null 2>&1; then
  MADE_MCP_STORE_PATH="$(cygpath -w "${MADE_MCP_STORE_PATH}")"
  export MADE_MCP_STORE_PATH
fi

config_status=0
made_embedded_load_config "${MADE_MCP_STORE_PATH}" || config_status=$?
if [[ "${config_status}" -eq 2 ]]; then
  exit 2
fi
if ! made_embedded_validate_runtime_config; then
  echo "MADE plugin: embedded authorization/search configuration is missing." >&2
  echo "MADE plugin: run made-setup (or /made:setup) for the selected store, or provide all four explicit environment overrides." >&2
  exit 2
fi

exec "${BINARY}"
