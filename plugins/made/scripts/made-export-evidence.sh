#!/usr/bin/env bash
# Export one ceremony's journal as a signed evidence bundle.
#
#   scripts/made-export-evidence.sh --ceremony <id> --out <file>
#
# The bundle carries the verified records and a signature over their head
# by the key made-setup created for this store. Anyone can judge the file
# with `made-mcp verify-evidence <file> --public-key <hex>`; the public
# key is printed by setup and by this script, and never the key file.
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

KEY_PATH="${MADE_EVIDENCE_SIGNING_KEY_PATH:-$(made_embedded_evidence_key_path "${MADE_MCP_STORE_PATH}")}"
if [[ ! -f "${KEY_PATH}" ]]; then
  made_embedded_error "no evidence signing key at ${KEY_PATH}."
  made_embedded_error "run made-setup (or /made:setup) to create one for this store, or set MADE_EVIDENCE_SIGNING_KEY_PATH."
  exit 2
fi

exec "${BINARY}" export-evidence "${MADE_MCP_STORE_PATH}" --key "${KEY_PATH}" "$@"
