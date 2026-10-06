#!/usr/bin/env bash
# Approve a human guard from your own terminal.
#
# The agent's MCP session cannot record this decision when the launcher runs
# with MADE_HUMAN_APPROVAL_SOURCE=terminal (the plugin default). This wrapper
# finds the same binary, store and authorization policy the launcher uses and
# runs `made-mcp approve-guard`, which shows what is being approved and asks.
#
#   scripts/made-approve.sh --ceremony <id> --guard <name> --role <role> [--reason <text>]
#
# Exit codes: 0 recorded, 3 declined at the prompt, 2 usage or not a terminal,
# 1 the store, the policy or the engine refused.
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=made-embedded-config.sh
source "${PLUGIN_ROOT}/scripts/made-embedded-config.sh"

BINARY="${MADE_MCP_BIN:-${PLUGIN_ROOT}/bin/made-mcp}"
if [[ -n "${MADE_MCP_BIN:-}" && ! -x "${MADE_MCP_BIN}" ]]; then
  made_embedded_error "MADE_MCP_BIN is set to '${MADE_MCP_BIN}', which is not executable."
  exit 127
fi
if [[ ! -x "${BINARY}" ]] && PATH_BINARY="$(command -v made-mcp 2>/dev/null)"; then
  BINARY="${PATH_BINARY}"
fi
if [[ ! -x "${BINARY}" ]]; then
  made_embedded_error "no made-mcp executable found; run /made:setup in Claude Code or the made-setup skill in Codex."
  exit 127
fi

MADE_MCP_STORE_PATH="$(made_embedded_store_path)"
export MADE_MCP_STORE_PATH

config_status=0
made_embedded_load_config "${MADE_MCP_STORE_PATH}" || config_status=$?
if [[ "${config_status}" -eq 2 ]]; then
  exit 2
fi
if ! made_embedded_validate_runtime_config; then
  made_embedded_error "embedded authorization configuration is missing; run made-setup (or /made:setup) for the selected store."
  exit 2
fi

if [[ ! -t 0 || ! -t 1 ]]; then
  made_embedded_error "made-approve.sh must be run by a person at an interactive terminal; it does not take answers from a pipe."
  exit 2
fi

exec "${BINARY}" approve-guard "${MADE_MCP_STORE_PATH}" "$@"
