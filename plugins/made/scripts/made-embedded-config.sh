#!/usr/bin/env bash

# Shared, deliberately boring configuration handling for the embedded plugin.
# This file is sourced by the POSIX launcher and by made-configure-embedded.sh.
# It never creates credentials: only the setup script may do that.

MADE_EMBEDDED_CONFIG_VARS=(
  MADE_AUTH_POLICY_ID
  MADE_AUTH_TRUSTED_HOST_ID
  MADE_CEREMONY_STORE_ID
  MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY
)

made_embedded_error() {
  echo "MADE plugin: $*" >&2
}

made_embedded_config_root() {
  printf '%s\n' "${MADE_SETUP_CONFIG_ROOT:-${XDG_CONFIG_HOME:-${HOME}/.config}/underpass-made/embedded}"
}

made_embedded_store_path() {
  if [[ -n "${MADE_MCP_STORE_PATH:-}" ]]; then
    printf '%s\n' "${MADE_MCP_STORE_PATH}"
    return 0
  fi

  local state_root="${XDG_STATE_HOME:-${HOME}/.local/state}/underpass-made"
  local sqlite="${state_root}/ceremonies.sqlite3"
  local legacy="${state_root}/ceremonies.redb"
  mkdir -p "${state_root}"
  if [[ ! -e "${sqlite}" && -e "${legacy}" ]]; then
    made_embedded_error "legacy Redb store found at ${legacy}."
    made_embedded_error "convert it before upgrading with made-mcp v0.2.0:"
    made_embedded_error "  made-mcp share-store '${legacy}'"
    made_embedded_error "the original is kept as a backup; no new store was created."
    return 2
  fi
  printf '%s\n' "${sqlite}"
}

made_embedded_store_digest() {
  local store="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    printf '%s' "${store}" | sha256sum | cut -c1-16
  elif command -v shasum >/dev/null 2>&1; then
    printf '%s' "${store}" | shasum -a 256 | cut -c1-16
  else
    made_embedded_error "sha256sum or shasum is required to address private setup configuration."
    return 2
  fi
}

made_embedded_config_path() {
  local store="$1"
  local digest
  digest="$(made_embedded_store_digest "${store}")" || return
  printf '%s/%s.env\n' "$(made_embedded_config_root)" "${digest}"
}

made_embedded_file_owner() {
  stat -c '%u' "$1" 2>/dev/null || stat -f '%u' "$1"
}

made_embedded_file_mode() {
  stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"
}

made_embedded_validate_identity() {
  local name="$1"
  local value="$2"
  if [[ -z "${value}" || "${value}" == *$'\n'* || "${value}" == *$'\r'* || "${value}" == *'='* || "${value}" =~ [[:space:]] ]]; then
    made_embedded_error "${name} is empty or contains unsupported whitespace/configuration characters."
    return 2
  fi
}

made_embedded_validate_key() {
  local value="$1"
  if [[ ! "${value}" =~ ^[[:xdigit:]]{64}$ ]]; then
    made_embedded_error "MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY must be exactly 32 bytes encoded as 64 hexadecimal characters."
    return 2
  fi
}

made_embedded_validate_runtime_config() {
  made_embedded_validate_identity MADE_AUTH_POLICY_ID "${MADE_AUTH_POLICY_ID:-}" || return
  made_embedded_validate_identity MADE_AUTH_TRUSTED_HOST_ID "${MADE_AUTH_TRUSTED_HOST_ID:-}" || return
  made_embedded_validate_identity MADE_CEREMONY_STORE_ID "${MADE_CEREMONY_STORE_ID:-}" || return
  made_embedded_validate_key "${MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY:-}" || return
}

made_embedded_read_config() {
  local store="$1"
  local config_path
  config_path="$(made_embedded_config_path "${store}")" || return
  if [[ ! -e "${config_path}" ]]; then
    return 1
  fi
  if [[ ! -r "${config_path}" ]]; then
    made_embedded_error "private setup configuration is unreadable at ${config_path}."
    made_embedded_error "repair it with made-setup; the launcher will not create or replace it."
    return 2
  fi
  if [[ -L "${config_path}" ]]; then
    made_embedded_error "private setup configuration must be a regular owner-owned file."
    made_embedded_error "repair it with made-setup; the launcher will not follow configuration links."
    return 2
  fi
  if [[ "$(made_embedded_file_owner "${config_path}")" != "$(id -u)" ]]; then
    made_embedded_error "private setup configuration is not owned by the current user."
    made_embedded_error "repair it with made-setup; the launcher will not use it."
    return 2
  fi
  local mode="$(made_embedded_file_mode "${config_path}")"
  if [[ "${mode}" != "600" && "${mode}" != "400" ]]; then
    made_embedded_error "private setup configuration must be owner-readable only (mode 600 or 400)."
    made_embedded_error "repair it with made-setup; the launcher will not use it."
    return 2
  fi

  local line key value
  local seen_policy=0 seen_host=0 seen_store=0 seen_key=0
  while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ -z "${line}" || "${line}" == \#* ]] && continue
    if [[ "${line}" != MADE_*=* ]]; then
      made_embedded_error "private setup configuration is malformed at ${config_path}."
      made_embedded_error "repair it with made-setup; the launcher will not guess missing values."
      return 2
    fi
    key="${line%%=*}"
    value="${line#*=}"
    case "${key}" in
      MADE_AUTH_POLICY_ID) seen_policy=$((seen_policy + 1)); MADE_CONFIG_AUTH_POLICY_ID="${value}" ;;
      MADE_AUTH_TRUSTED_HOST_ID) seen_host=$((seen_host + 1)); MADE_CONFIG_AUTH_TRUSTED_HOST_ID="${value}" ;;
      MADE_CEREMONY_STORE_ID) seen_store=$((seen_store + 1)); MADE_CONFIG_CEREMONY_STORE_ID="${value}" ;;
      MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY) seen_key=$((seen_key + 1)); MADE_CONFIG_CEREMONY_SEARCH_CURSOR_HMAC_KEY="${value}" ;;
      *)
        made_embedded_error "private setup configuration contains an unknown key at ${config_path}."
        made_embedded_error "repair it with made-setup; the launcher will not use it."
        return 2
        ;;
    esac
  done <"${config_path}" || {
    made_embedded_error "private setup configuration could not be read at ${config_path}."
    made_embedded_error "repair it with made-setup; the launcher will not use it."
    return 2
  }

  if [[ "${seen_policy}" -ne 1 || "${seen_host}" -ne 1 || "${seen_store}" -ne 1 || "${seen_key}" -ne 1 ]]; then
    made_embedded_error "private setup configuration is incomplete at ${config_path}."
    made_embedded_error "repair it with made-setup; the launcher will not generate a replacement."
    return 2
  fi
  made_embedded_validate_identity MADE_AUTH_POLICY_ID "${MADE_CONFIG_AUTH_POLICY_ID}" || return 2
  made_embedded_validate_identity MADE_AUTH_TRUSTED_HOST_ID "${MADE_CONFIG_AUTH_TRUSTED_HOST_ID}" || return 2
  made_embedded_validate_identity MADE_CEREMONY_STORE_ID "${MADE_CONFIG_CEREMONY_STORE_ID}" || return 2
  made_embedded_validate_key "${MADE_CONFIG_CEREMONY_SEARCH_CURSOR_HMAC_KEY}" || return 2
}

made_embedded_load_config() {
  local store="$1"
  made_embedded_read_config "${store}"
  local status=$?
  [[ "${status}" -eq 0 ]] || return "${status}"

  # Explicit launch-environment values win over the private setup file. This
  # is intentional for tests and operator-managed registrations; setup never
  # overwrites an existing file merely because an override was supplied.
  [[ -n "${MADE_AUTH_POLICY_ID:-}" ]] || export MADE_AUTH_POLICY_ID="${MADE_CONFIG_AUTH_POLICY_ID}"
  [[ -n "${MADE_AUTH_TRUSTED_HOST_ID:-}" ]] || export MADE_AUTH_TRUSTED_HOST_ID="${MADE_CONFIG_AUTH_TRUSTED_HOST_ID}"
  [[ -n "${MADE_CEREMONY_STORE_ID:-}" ]] || export MADE_CEREMONY_STORE_ID="${MADE_CONFIG_CEREMONY_STORE_ID}"
  [[ -n "${MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY:-}" ]] || export MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY="${MADE_CONFIG_CEREMONY_SEARCH_CURSOR_HMAC_KEY}"
}

made_embedded_write_config() {
  local store="$1"
  local config_path
  config_path="$(made_embedded_config_path "${store}")" || return
  local config_root
  config_root="$(dirname "${config_path}")"
  umask 077
  mkdir -p "${config_root}"
  chmod 700 "$(dirname "${config_root}")" "${config_root}" 2>/dev/null || true
  local temporary="${config_path}.tmp.$$"
  (
    umask 077
    printf '%s\n' '# MADE embedded host configuration; managed by made-setup.' >"${temporary}"
    printf 'MADE_AUTH_POLICY_ID=%s\n' "${MADE_AUTH_POLICY_ID}" >>"${temporary}"
    printf 'MADE_AUTH_TRUSTED_HOST_ID=%s\n' "${MADE_AUTH_TRUSTED_HOST_ID}" >>"${temporary}"
    printf 'MADE_CEREMONY_STORE_ID=%s\n' "${MADE_CEREMONY_STORE_ID}" >>"${temporary}"
    printf 'MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY=%s\n' "${MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY}" >>"${temporary}"
    chmod 600 "${temporary}"
    mv -f "${temporary}" "${config_path}"
  ) || {
    rm -f "${temporary}"
    made_embedded_error "could not write private setup configuration at ${config_path}."
    return 2
  }
  printf '%s\n' "${config_path}"
}
