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

SCRATCH="$(mktemp -d)"
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

INITIALIZE="$(head -n 1 "${ROOT_DIR}/tests/plugin/made-smoke.jsonl")"
RESPONSE="$(printf '%s\n' "${INITIALIZE}" | \
  MADE_MCP_STORE_PATH="${SCRATCH}/ceremonies.sqlite3" \
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
