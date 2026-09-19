#!/usr/bin/env bash
# Build and smoke a native made-console release candidate without publishing it.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/console"
TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT_DIR}/target}"

cd "${ROOT_DIR}"
rm -rf "${DIST_DIR}"
mkdir -p "${DIST_DIR}"

WORKSPACE_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
if [[ -z "${WORKSPACE_VERSION}" ]]; then
  echo "MADE console package: could not read the workspace version" >&2
  exit 1
fi

TAG_NAME="${GITHUB_REF_NAME:-}"
if [[ "${TAG_NAME}" == v* ]]; then
  TAG_VERSION="${TAG_NAME#v}"
  if [[ "${TAG_VERSION}" != "${WORKSPACE_VERSION}" ]]; then
    echo "MADE console package: tag ${TAG_NAME} does not match workspace version ${WORKSPACE_VERSION}" >&2
    exit 1
  fi
  PACKAGE_VERSION="${WORKSPACE_VERSION}"
else
  PACKAGE_VERSION="${WORKSPACE_VERSION}+$(git rev-parse --short HEAD)"
fi

SYSTEM="$(uname -s)-$(uname -m)"
case "${SYSTEM}" in
  Linux-x86_64)
    TARGET_TRIPLE="x86_64-unknown-linux-gnu"
    BINARY_NAME="made-console"
    ;;
  Linux-aarch64)
    TARGET_TRIPLE="aarch64-unknown-linux-gnu"
    BINARY_NAME="made-console"
    ;;
  Darwin-arm64)
    TARGET_TRIPLE="aarch64-apple-darwin"
    BINARY_NAME="made-console"
    ;;
  MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64)
    TARGET_TRIPLE="x86_64-pc-windows-msvc"
    BINARY_NAME="made-console.exe"
    ;;
  *)
    echo "MADE console package: unsupported release platform ${SYSTEM}" >&2
    exit 1
    ;;
esac

# These reads exercise the exact manifests crates.io will consume. They do not
# upload and do not require the new dependency versions to exist remotely.
cargo package --list -p made-client >/dev/null
cargo package --list -p made-console >/dev/null
cargo build --release --locked -p made-console

BINARY="${TARGET_DIR}/release/${BINARY_NAME}"
if [[ ! -f "${BINARY}" ]]; then
  echo "MADE console package: build did not produce ${BINARY}" >&2
  exit 1
fi

"${BINARY}" --version | grep -Fx "made-console ${WORKSPACE_VERSION}" >/dev/null
"${BINARY}" --help | grep -F "Usage: made-console" >/dev/null
"${BINARY}" artifact --help | grep -F "export" >/dev/null

ASSET_NAME="made-console-v${PACKAGE_VERSION}-${TARGET_TRIPLE}"
[[ "${BINARY_NAME}" == *.exe ]] && ASSET_NAME="${ASSET_NAME}.exe"
cp "${BINARY}" "${DIST_DIR}/${ASSET_NAME}"
[[ "${BINARY_NAME}" != *.exe ]] && chmod +x "${DIST_DIR}/${ASSET_NAME}"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "${DIST_DIR}" && sha256sum "${ASSET_NAME}" >"${ASSET_NAME}.sha256")
else
  (cd "${DIST_DIR}" && shasum -a 256 "${ASSET_NAME}" >"${ASSET_NAME}.sha256")
fi

echo "MADE console candidate: ${DIST_DIR}/${ASSET_NAME}"
echo "MADE console candidate version: ${PACKAGE_VERSION}"
