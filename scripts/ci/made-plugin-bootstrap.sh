#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLUGIN_DIR="${ROOT_DIR}/plugins/made"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) TARGET="x86_64-unknown-linux-gnu"; BINARY_NAME="made-mcp" ;;
  Linux-aarch64) TARGET="aarch64-unknown-linux-gnu"; BINARY_NAME="made-mcp" ;;
  Darwin-arm64) TARGET="aarch64-apple-darwin"; BINARY_NAME="made-mcp" ;;
  MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64)
    TARGET="x86_64-pc-windows-msvc"; BINARY_NAME="made-mcp.exe"
    ;;
  *) echo "MADE plugin bootstrap: unsupported CI platform" >&2; exit 1 ;;
esac

SOURCE_BINARY="${PLUGIN_DIR}/bin/${BINARY_NAME}"
[[ -x "${SOURCE_BINARY}" ]] || {
  echo "MADE plugin bootstrap: build the plugin binary before this test" >&2
  exit 1
}

mkdir -p "${ROOT_DIR}/tmp"
SCRATCH="$(mktemp -d "${ROOT_DIR}/tmp/plugin-bootstrap.XXXXXX")"
trap 'rm -rf "${SCRATCH}"' EXIT
cp -R "${PLUGIN_DIR}" "${SCRATCH}/made"
rm -rf "${SCRATCH}/made/bin"
mkdir -p "${SCRATCH}/fake-bin"
cp "${ROOT_DIR}/tests/plugin/fake-release-curl.sh" "${SCRATCH}/fake-bin/curl"
chmod +x "${SCRATCH}/fake-bin/curl"

REQUESTS="${SCRATCH}/requests.txt"
: >"${REQUESTS}"
TEST_PATH="${SCRATCH}/fake-bin:/usr/bin:/bin"
if PATH="${TEST_PATH}" command -v cargo >/dev/null 2>&1; then
  echo "MADE plugin bootstrap: isolated PATH unexpectedly contains cargo" >&2
  exit 1
fi

MADE_INSTALL_DIR="${SCRATCH}/made/bin" \
MADE_FAKE_CURL_SOURCE="${SOURCE_BINARY}" \
MADE_FAKE_CURL_REQUESTS="${REQUESTS}" \
PATH="${TEST_PATH}" \
  "${SCRATCH}/made/scripts/made-install-binary.sh"

VERSION="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
  "${PLUGIN_DIR}/.codex-plugin/plugin.json" | head -n 1)"
VERSION="${VERSION%%+*}"
ASSET="made-mcp-v${VERSION}-${TARGET}"
[[ "${BINARY_NAME}" == "made-mcp.exe" ]] && ASSET="${ASSET}.exe"
EXPECTED="https://github.com/underpass-ai/made/releases/download/v${VERSION}/${ASSET}"

REQUEST_COUNT="$(wc -l <"${REQUESTS}" | tr -d ' ')"
REQUEST_ONE="$(sed -n '1p' "${REQUESTS}")"
REQUEST_TWO="$(sed -n '2p' "${REQUESTS}")"
[[ "${REQUEST_COUNT}" -eq 2 ]] || {
  echo "MADE plugin bootstrap: expected binary and checksum downloads" >&2
  exit 1
}
[[ "${REQUEST_ONE}" == "${EXPECTED}" && "${REQUEST_TWO}" == "${EXPECTED}.sha256" ]] || {
  echo "MADE plugin bootstrap: installer requested the wrong release assets" >&2
  sed -n '1,4p' "${REQUESTS}" >&2
  exit 1
}

INSTALLED="${SCRATCH}/made/bin/${BINARY_NAME}"
"${INSTALLED}" --version | grep -F "made-mcp ${VERSION}" >/dev/null

export MADE_MCP_STORE_PATH="${SCRATCH}/ceremonies.sqlite3"
if command -v cygpath >/dev/null 2>&1; then
  export MADE_MCP_STORE_PATH="$(cygpath -w "${MADE_MCP_STORE_PATH}")"
fi
export MADE_SETUP_CONFIG_ROOT="${SCRATCH}/host-config"
unset MADE_AUTH_POLICY_ID MADE_AUTH_TRUSTED_HOST_ID MADE_CEREMONY_STORE_ID MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY
MADE_MCP_BIN="${INSTALLED}" "${SCRATCH}/made/scripts/made-configure-embedded.sh" >/dev/null

CONFIG_FILE="$(find "${SCRATCH}/host-config" -type f -name '*.env' -print -quit)"
[[ -n "${CONFIG_FILE}" ]] || {
  echo "MADE plugin bootstrap: setup did not persist private host configuration" >&2
  exit 1
}
if command -v cygpath >/dev/null 2>&1; then
  [[ -f "${CONFIG_FILE}" ]] || {
    echo "MADE plugin bootstrap: private host configuration is not a regular file" >&2
    exit 1
  }
else
  [[ "$(stat -c '%a' "${CONFIG_FILE}" 2>/dev/null || stat -f '%Lp' "${CONFIG_FILE}")" == "600" ]] || {
    echo "MADE plugin bootstrap: private host configuration is not owner-only" >&2
    exit 1
  }
fi
CURSOR_KEY="$(sed -n 's/^MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY=//p' "${CONFIG_FILE}")"
[[ "${#CURSOR_KEY}" -eq 64 ]] || {
  echo "MADE plugin bootstrap: setup did not persist a 32-byte cursor key" >&2
  exit 1
}
if MADE_MCP_BIN="${INSTALLED}" "${SCRATCH}/made/scripts/made-configure-embedded.sh" \
  | grep -Fq "${CURSOR_KEY}"; then
  echo "MADE plugin bootstrap: setup receipt leaked the cursor key" >&2
  exit 1
fi
CURSOR_KEY_AFTER="$(sed -n 's/^MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY=//p' "${CONFIG_FILE}")"
[[ "${CURSOR_KEY}" == "${CURSOR_KEY_AFTER}" ]] || {
  echo "MADE plugin bootstrap: setup rotated the cursor key on restart" >&2
  exit 1
}

export MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY="$(printf 'bb%.0s' {1..32})"
INITIALIZE="$(head -n 1 "${ROOT_DIR}/tests/plugin/made-smoke.jsonl")"
OVERRIDE_RESPONSE="$(printf '%s\n' "${INITIALIZE}" | \
  "${SCRATCH}/made/scripts/run-embedded-mcp.sh")"
[[ "${OVERRIDE_RESPONSE}" == *'"serverInfo"'* ]] || {
  echo "MADE plugin bootstrap: explicit environment override did not start the launcher" >&2
  exit 1
}
unset MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY

printf 'malformed line\n' >>"${CONFIG_FILE}"
if "${SCRATCH}/made/scripts/run-embedded-mcp.sh" </dev/null >/dev/null 2>&1; then
  echo "MADE plugin bootstrap: accepted malformed private configuration" >&2
  exit 1
fi
CONFIG_CLEAN="${CONFIG_FILE}.clean"
sed '$d' "${CONFIG_FILE}" >"${CONFIG_CLEAN}"
chmod 600 "${CONFIG_CLEAN}" 2>/dev/null || true
mv "${CONFIG_CLEAN}" "${CONFIG_FILE}"

RESPONSE="$(printf '%s\n' "${INITIALIZE}" | \
  "${SCRATCH}/made/scripts/run-embedded-mcp.sh")"
[[ "${RESPONSE}" == *'"serverInfo"'* ]] || {
  echo "MADE plugin bootstrap: verified binary did not start through the launcher" >&2
  exit 1
}

if MADE_SETUP_FORCE=1 \
  MADE_INSTALL_DIR="${SCRATCH}/bad-bin" \
  MADE_FAKE_CURL_SOURCE="${SOURCE_BINARY}" \
  MADE_FAKE_CURL_REQUESTS="${REQUESTS}" \
  MADE_FAKE_CURL_BAD_CHECKSUM=1 \
  PATH="${TEST_PATH}" \
  "${SCRATCH}/made/scripts/made-install-binary.sh" >/dev/null 2>&1; then
  echo "MADE plugin bootstrap: accepted a mismatched checksum" >&2
  exit 1
fi
[[ ! -e "${SCRATCH}/bad-bin/${BINARY_NAME}" ]] || {
  echo "MADE plugin bootstrap: installed a binary after checksum failure" >&2
  exit 1
}

echo "MADE marketplace bootstrap passed"
