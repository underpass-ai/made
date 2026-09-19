#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "${ROOT_DIR}/tmp"
SCRATCH="$(mktemp -d "${ROOT_DIR}/tmp/plugin-package-assets.XXXXXX")"
trap 'rm -rf "${SCRATCH}"' EXIT

TEST_ROOT="${SCRATCH}/repo"
mkdir -p "${TEST_ROOT}/scripts/plugin" "${TEST_ROOT}/scripts/ci" \
  "${TEST_ROOT}/tests/plugin"
cp "${ROOT_DIR}/Cargo.toml" "${TEST_ROOT}/Cargo.toml"
cp -R "${ROOT_DIR}/plugins" "${TEST_ROOT}/plugins"
rm -rf "${TEST_ROOT}/plugins/made/bin"
cp "${ROOT_DIR}/scripts/plugin/package-made-plugin.sh" "${TEST_ROOT}/scripts/plugin/"
cp "${ROOT_DIR}/scripts/ci/plugin-bundle-assets.py" "${TEST_ROOT}/scripts/ci/"

cat >"${TEST_ROOT}/scripts/plugin/build-local-made-plugin.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BINARY_NAME="made-mcp"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) BINARY_NAME="made-mcp.exe" ;;
esac
[[ "${OS:-}" == "Windows_NT" ]] && BINARY_NAME="made-mcp.exe"
mkdir -p "${ROOT_DIR}/plugins/made/bin"
printf '#!/usr/bin/env bash\necho fixture-made-mcp\n' \
  >"${ROOT_DIR}/plugins/made/bin/${BINARY_NAME}"
chmod +x "${ROOT_DIR}/plugins/made/bin/${BINARY_NAME}"
EOF
chmod +x "${TEST_ROOT}/scripts/plugin/build-local-made-plugin.sh"

WORKSPACE_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' \
  "${TEST_ROOT}/Cargo.toml" | head -1)"
[[ -n "${WORKSPACE_VERSION}" ]]
GITHUB_REF_NAME="v${WORKSPACE_VERSION}" \
  bash "${TEST_ROOT}/scripts/plugin/package-made-plugin.sh" >/dev/null

ARCHIVE="$(find "${TEST_ROOT}/dist/plugin" -maxdepth 1 -name 'made-plugin-*.tar.gz' -print -quit)"
STANDALONE="$(find "${TEST_ROOT}/dist/plugin" -maxdepth 1 -name 'made-mcp-v*' ! -name '*.sha256' -print -quit)"
[[ -n "${ARCHIVE}" && -n "${STANDALONE}" ]]
mkdir -p "${SCRATCH}/unpacked"
tar -xzf "${ARCHIVE}" -C "${SCRATCH}/unpacked"
python3 "${TEST_ROOT}/scripts/ci/plugin-bundle-assets.py" \
  "${SCRATCH}/unpacked/made" >"${SCRATCH}/verified.json"

PACKAGED_BINARY="$(find "${SCRATCH}/unpacked/made/bin" -maxdepth 1 \
  -name 'made-mcp*' -type f -print -quit)"
[[ -n "${PACKAGED_BINARY}" ]]
cmp "${PACKAGED_BINARY}" "${STANDALONE}"
grep -Fq '9cb1766d30c0f1d5e81bdf1e2831bb002bc750665a1762a0ff70207bdd456115' \
  "${SCRATCH}/verified.json"
[[ "$(find "${SCRATCH}/unpacked/made/skills" -mindepth 1 -maxdepth 1 -type d | wc -l)" -eq 3 ]]

rm "${SCRATCH}/unpacked/made/assets/made-cuatro-voces.png"
if python3 "${TEST_ROOT}/scripts/ci/plugin-bundle-assets.py" \
  "${SCRATCH}/unpacked/made" >"${SCRATCH}/missing.stdout" 2>"${SCRATCH}/missing.stderr"; then
  echo "MADE plugin package assets test accepted a missing manifest artwork" >&2
  exit 1
fi
grep -Fq 'points at missing file assets/made-cuatro-voces.png' "${SCRATCH}/missing.stderr"

echo "MADE plugin package assets test passed"
