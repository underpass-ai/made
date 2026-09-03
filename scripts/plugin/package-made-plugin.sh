#!/usr/bin/env bash
# Package the MADE plugin bundle for Codex and Claude Code.
#
# Single source of truth for the version: the workspace Cargo.toml. On a
# `v*` tag (CI release) the tag must match the workspace version exactly —
# a release never lies about what it contains. Outside a tag the package
# gets `+<short-sha>` build metadata so a dev tarball can never pass for
# the release it merely resembles.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLUGIN_DIR="${ROOT_DIR}/plugins/made"
DIST_DIR="${ROOT_DIR}/dist/plugin"
STAGE_DIR="${DIST_DIR}/stage"

cd "${ROOT_DIR}"

# The release job globs `dist/plugin/*`, so anything left here from an
# earlier build — or accidentally committed — would be published as if it
# belonged to this version. Start from an empty directory so the archives
# that leave this script are exactly the ones it made.
rm -rf "${DIST_DIR}"

WORKSPACE_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
if [[ -z "${WORKSPACE_VERSION}" ]]; then
  echo "MADE plugin package: could not read the workspace version" >&2
  exit 1
fi

TAG_NAME="${GITHUB_REF_NAME:-}"
if [[ "${TAG_NAME}" == v* ]]; then
  TAG_VERSION="${TAG_NAME#v}"
  if [[ "${TAG_VERSION}" != "${WORKSPACE_VERSION}" ]]; then
    echo "MADE plugin package: tag ${TAG_NAME} does not match workspace version ${WORKSPACE_VERSION}" >&2
    exit 1
  fi
  PACKAGE_VERSION="${WORKSPACE_VERSION}"
else
  SHORT_SHA="$(git rev-parse --short HEAD)"
  PACKAGE_VERSION="${WORKSPACE_VERSION}+${SHORT_SHA}"
fi

# Build the embedded binary and place it at bin/made-mcp.
bash scripts/plugin/build-local-made-plugin.sh

SYSTEM="$(uname -s)-$(uname -m)"
case "${SYSTEM}" in
  Linux-x86_64)
    OS_LABEL="linux"
    ARCH_LABEL="x86_64"
    TARGET_TRIPLE="x86_64-unknown-linux-gnu"
    BINARY_NAME="made-mcp"
    ;;
  Linux-aarch64)
    OS_LABEL="linux"
    ARCH_LABEL="arm64"
    TARGET_TRIPLE="aarch64-unknown-linux-gnu"
    BINARY_NAME="made-mcp"
    ;;
  Darwin-arm64)
    OS_LABEL="macos"
    ARCH_LABEL="arm64"
    TARGET_TRIPLE="aarch64-apple-darwin"
    BINARY_NAME="made-mcp"
    ;;
  Darwin-x86_64)
    OS_LABEL="macos"
    ARCH_LABEL="x86_64"
    TARGET_TRIPLE="x86_64-apple-darwin"
    BINARY_NAME="made-mcp"
    ;;
  MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64)
    OS_LABEL="windows"
    ARCH_LABEL="x86_64"
    TARGET_TRIPLE="x86_64-pc-windows-msvc"
    BINARY_NAME="made-mcp.exe"
    ;;
  *)
    echo "MADE plugin package: unsupported release platform ${SYSTEM}" >&2
    exit 1
    ;;
esac

# Stamp the resolved version into both host manifests.
python3 - "${PACKAGE_VERSION}" <<'EOF'
import json
import pathlib
import sys

version = sys.argv[1]
plugin_dir = pathlib.Path("plugins/made")
for manifest in (".codex-plugin/plugin.json", ".claude-plugin/plugin.json"):
    path = plugin_dir / manifest
    data = json.loads(path.read_text())
    data["version"] = version
    path.write_text(json.dumps(data, indent=2) + "\n")
EOF

# Stage a clean copy named after the plugin so the tarball unpacks as
# `made/` on any host. `bin/` is gitignored, so it is copied explicitly.
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/made/bin"
cp -R "${PLUGIN_DIR}/.codex-plugin" "${STAGE_DIR}/made/.codex-plugin"
cp -R "${PLUGIN_DIR}/.claude-plugin" "${STAGE_DIR}/made/.claude-plugin"
cp -R "${PLUGIN_DIR}/.mcp.json" "${STAGE_DIR}/made/.mcp.json"
cp -R "${PLUGIN_DIR}/README.md" "${STAGE_DIR}/made/README.md"
cp -R "${PLUGIN_DIR}/claude" "${STAGE_DIR}/made/claude"
cp -R "${PLUGIN_DIR}/skills" "${STAGE_DIR}/made/skills"
cp -R "${PLUGIN_DIR}/scripts" "${STAGE_DIR}/made/scripts"
cp "${PLUGIN_DIR}/bin/made-mcp"* "${STAGE_DIR}/made/bin/"
chmod +x "${STAGE_DIR}/made/scripts/run-embedded-mcp.sh"
[[ -f "${STAGE_DIR}/made/bin/made-mcp" ]] && chmod +x "${STAGE_DIR}/made/bin/made-mcp"

ARCHIVE_NAME="made-plugin-${PACKAGE_VERSION}-${OS_LABEL}-${ARCH_LABEL}.tar.gz"
STANDALONE_NAME="made-mcp-v${PACKAGE_VERSION}-${TARGET_TRIPLE}"
[[ "${BINARY_NAME}" == "made-mcp.exe" ]] && STANDALONE_NAME="${STANDALONE_NAME}.exe"
mkdir -p "${DIST_DIR}"
tar -czf "${DIST_DIR}/${ARCHIVE_NAME}" -C "${STAGE_DIR}" made
cp "${PLUGIN_DIR}/bin/${BINARY_NAME}" "${DIST_DIR}/${STANDALONE_NAME}"
[[ "${BINARY_NAME}" == "made-mcp" ]] && chmod +x "${DIST_DIR}/${STANDALONE_NAME}"
rm -rf "${STAGE_DIR}"

checksum() {
  local filename="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "${DIST_DIR}" && sha256sum "${filename}" > "${filename}.sha256")
  else
    (cd "${DIST_DIR}" && shasum -a 256 "${filename}" > "${filename}.sha256")
  fi
}

checksum "${ARCHIVE_NAME}"
checksum "${STANDALONE_NAME}"

echo "MADE plugin package: ${DIST_DIR}/${ARCHIVE_NAME}"
echo "MADE standalone engine: ${DIST_DIR}/${STANDALONE_NAME}"
echo "MADE plugin package version: ${PACKAGE_VERSION}"
