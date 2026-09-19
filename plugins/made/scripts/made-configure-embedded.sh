#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=made-embedded-config.sh
source "${PLUGIN_ROOT}/scripts/made-embedded-config.sh"

binary="${MADE_MCP_BIN:-${PLUGIN_ROOT}/bin/made-mcp}"
if [[ -n "${MADE_MCP_BIN:-}" && ! -x "${MADE_MCP_BIN}" ]]; then
  made_embedded_error "MADE_MCP_BIN is set to '${MADE_MCP_BIN}', which is not executable."
  exit 127
fi
if [[ ! -x "${binary}" && -x "${PLUGIN_ROOT}/bin/made-mcp.exe" ]]; then
  binary="${PLUGIN_ROOT}/bin/made-mcp.exe"
fi
if [[ ! -x "${binary}" ]] && binary_from_path="$(command -v made-mcp 2>/dev/null || true)"; then
  binary="${binary_from_path}"
fi
if [[ ! -x "${binary}" ]]; then
  made_embedded_error "no made-mcp executable found; install the release binary first."
  exit 127
fi

store="$(made_embedded_store_path)"
if command -v cygpath >/dev/null 2>&1; then
  store="$(cygpath -w "${store}")"
fi

config_status=0
made_embedded_load_config "${store}" || config_status=$?
if [[ "${config_status}" -eq 2 ]]; then
  exit 2
fi

digest="$(made_embedded_store_digest "${store}")"
[[ -n "${MADE_AUTH_POLICY_ID:-}" ]] || MADE_AUTH_POLICY_ID="made-local-policy-${digest}"
[[ -n "${MADE_AUTH_TRUSTED_HOST_ID:-}" ]] || MADE_AUTH_TRUSTED_HOST_ID="made-local-host-${digest}"
[[ -n "${MADE_CEREMONY_STORE_ID:-}" ]] || MADE_CEREMONY_STORE_ID="made-local-store-${digest}"
if [[ -z "${MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY:-}" ]]; then
  if command -v openssl >/dev/null 2>&1; then
    MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY="$(openssl rand -hex 32)"
  elif command -v od >/dev/null 2>&1; then
    MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
  else
    made_embedded_error "a cryptographically secure random source is required to create the search cursor key."
    exit 2
  fi
fi
export MADE_AUTH_POLICY_ID MADE_AUTH_TRUSTED_HOST_ID MADE_CEREMONY_STORE_ID MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY
made_embedded_validate_runtime_config

config_path="$(made_embedded_config_path "${store}")"
if [[ ! -e "${config_path}" ]]; then
  made_embedded_write_config "${store}" >/dev/null
fi

# The key is never passed to a child as a command argument and the setup
# receipt is intentionally redacted. The bootstrap command itself only needs
# the non-secret policy and trusted-host identities.
if ! bootstrap_output="$("${binary}" bootstrap-authorization "${store}" \
    --policy-id "${MADE_AUTH_POLICY_ID}" \
    --trusted-host-id "${MADE_AUTH_TRUSTED_HOST_ID}" 2>&1)"; then
  made_embedded_error "authorization bootstrap failed for the selected store."
  made_embedded_error "the private setup configuration was not replaced; inspect the made-mcp error and repair the store explicitly."
  printf '%s\n' "${bootstrap_output}" >&2
  exit 1
fi

echo "MADE setup: embedded store configured and authorization bootstrap completed."
echo "MADE setup: persistent search cursor configured (key redacted)."
echo "MADE setup: Codex and Claude can share this setup through the single MADE MCP registration."
