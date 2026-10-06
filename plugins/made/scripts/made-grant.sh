#!/usr/bin/env bash
# Decide what the host may do, from your own terminal.
#
# Bootstrapping the store opened its authorization policy and granted nothing:
# every business tool is refused until a grant names its action. The agent's
# MCP session cannot issue that grant under the plugin's default `core`
# profile, on purpose. This wrapper finds the same binary, store and policy
# the launcher uses and runs `made-mcp grant`, which shows what is about to be
# granted and asks.
#
#   scripts/made-grant.sh --profile core                 # the ordinary route
#   scripts/made-grant.sh --actions design_ceremony,publish_ceremony_definition
#   scripts/made-grant.sh --show                         # what the host holds
#
# Exit codes: 0 recorded or shown, 3 declined at the prompt, 2 usage or not a
# terminal, 1 the store or the policy refused.
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

show_only=0
for argument in "$@"; do
  [[ "${argument}" == "--show" ]] && show_only=1
done
if [[ "${show_only}" -eq 0 && ( ! -t 0 || ! -t 1 ) ]]; then
  made_embedded_error "made-grant.sh must be run by a person at an interactive terminal; it does not take answers from a pipe. Use --show to read what the host holds."
  exit 2
fi

exec "${BINARY}" grant "${MADE_MCP_STORE_PATH}" "$@"
