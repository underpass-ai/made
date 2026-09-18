#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "${SCRATCH}"' EXIT

TEST_ROOT="${SCRATCH}/repo"
FAKE_BIN="${SCRATCH}/fake-bin"
EXTERNAL_TARGET="${SCRATCH}/external-target"
mkdir -p "${TEST_ROOT}/scripts/plugin" "${TEST_ROOT}/plugins/made" "${FAKE_BIN}"
cp "${ROOT_DIR}/scripts/plugin/build-local-made-plugin.sh" \
  "${TEST_ROOT}/scripts/plugin/build-local-made-plugin.sh"

cat >"${FAKE_BIN}/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  metadata)
    printf '{"target_directory":"%s"}\n' "${FAKE_CARGO_TARGET_DIR}"
    ;;
  build)
    if [[ "${FAKE_CARGO_SKIP_BINARY:-0}" != "1" ]]; then
      mkdir -p "${FAKE_CARGO_TARGET_DIR}/release"
      printf '#!/usr/bin/env bash\necho fake-made-mcp\n' \
        >"${FAKE_CARGO_TARGET_DIR}/release/made-mcp${FAKE_CARGO_BINARY_SUFFIX:-}"
      chmod +x \
        "${FAKE_CARGO_TARGET_DIR}/release/made-mcp${FAKE_CARGO_BINARY_SUFFIX:-}"
    fi
    ;;
  *)
    echo "fake cargo: unexpected command ${*}" >&2
    exit 1
    ;;
esac
EOF
chmod +x "${FAKE_BIN}/cargo"

TEST_PATH="${FAKE_BIN}:${PATH}"
mkdir -p "${EXTERNAL_TARGET}/release"
printf '#!/usr/bin/env bash\necho stale-windows-binary\n' \
  >"${EXTERNAL_TARGET}/release/made-mcp.exe"
chmod +x "${EXTERNAL_TARGET}/release/made-mcp.exe"
env -u OS \
  FAKE_CARGO_TARGET_DIR="${EXTERNAL_TARGET}" \
  PATH="${TEST_PATH}" \
  bash "${TEST_ROOT}/scripts/plugin/build-local-made-plugin.sh" >/dev/null

INSTALLED="${TEST_ROOT}/plugins/made/bin/made-mcp"
[[ -x "${INSTALLED}" ]] || {
  echo "MADE plugin target-dir test: external-target binary was not installed" >&2
  exit 1
}
[[ "$("${INSTALLED}")" == "fake-made-mcp" ]] || {
  echo "MADE plugin target-dir test: installed the wrong binary" >&2
  exit 1
}
[[ ! -e "${TEST_ROOT}/target/release/made-mcp" ]] || {
  echo "MADE plugin target-dir test: fake build unexpectedly used the repository target" >&2
  exit 1
}

rm -rf "${TEST_ROOT}/plugins/made/bin" "${EXTERNAL_TARGET}"
mkdir -p "${EXTERNAL_TARGET}/release"
printf '#!/usr/bin/env bash\necho stale-unix-binary\n' \
  >"${EXTERNAL_TARGET}/release/made-mcp"
chmod +x "${EXTERNAL_TARGET}/release/made-mcp"
FAKE_CARGO_BINARY_SUFFIX=.exe \
  FAKE_CARGO_TARGET_DIR="${EXTERNAL_TARGET}" \
  OS=Windows_NT \
  PATH="${TEST_PATH}" \
  bash "${TEST_ROOT}/scripts/plugin/build-local-made-plugin.sh" >/dev/null

WINDOWS_INSTALLED="${TEST_ROOT}/plugins/made/bin/made-mcp.exe"
[[ -x "${WINDOWS_INSTALLED}" ]] || {
  echo "MADE plugin target-dir test: Windows binary was not installed" >&2
  exit 1
}
[[ "$("${WINDOWS_INSTALLED}")" == "fake-made-mcp" ]] || {
  echo "MADE plugin target-dir test: installed the wrong Windows binary" >&2
  exit 1
}

rm -rf "${TEST_ROOT}/plugins/made/bin" "${EXTERNAL_TARGET}"
if failure="$(
  FAKE_CARGO_TARGET_DIR="${EXTERNAL_TARGET}" \
    FAKE_CARGO_SKIP_BINARY=1 \
    PATH="${TEST_PATH}" \
    bash "${TEST_ROOT}/scripts/plugin/build-local-made-plugin.sh" 2>&1
)"; then
  echo "MADE plugin target-dir test: missing build output was accepted" >&2
  exit 1
fi
if [[ "${failure}" != *"built binary not found"* ]]; then
  echo "MADE plugin target-dir test: missing output error was not actionable" >&2
  printf '%s\n' "${failure}" >&2
  exit 1
fi
[[ ! -e "${TEST_ROOT}/plugins/made/bin/made-mcp" ]] || {
  echo "MADE plugin target-dir test: missing output copied a binary" >&2
  exit 1
}

echo "MADE plugin target-dir test passed"
