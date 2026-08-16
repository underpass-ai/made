#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

assert_embedded_boundary() {
  local label="$1"
  shift
  local forbidden=()
  local package
  local name

  while IFS= read -r package; do
    name="${package%% *}"
    case "${name}" in
      async-nats|made-mcp-proto|made-proto|prost|prost-types|sqlx|tonic)
        forbidden+=("${name}")
        ;;
    esac
  done < <(cargo tree --locked "$@" -e normal --prefix none --format '{p}')

  if ((${#forbidden[@]} > 0)); then
    echo "${label} crosses the remote-infrastructure dependency boundary:" >&2
    for name in "${forbidden[@]}"; do
      echo "- ${name}" >&2
    done
    exit 1
  fi

  echo "${label} dependency boundary passed"
}

assert_embedded_boundary "made-embedded" -p made-embedded
assert_embedded_boundary \
  "made-mcp embedded backend" \
  -p made-mcp \
  --no-default-features \
  --features embedded

# The SQLite engine is opt-in precisely so the default embedded build stays
# pure Rust with no C toolchain. A C dependency reaching it — through a
# feature-unification accident, a `default = ["sqlite"]` slip, anything — is
# the regression this check exists to catch.
assert_no_c_engine() {
  local label="$1"
  shift

  if cargo tree --locked "$@" -e normal --prefix none --format '{p}' \
    | grep -qE '^(rusqlite|libsqlite3-sys) '; then
    echo "${label} carries the SQLite C engine; it must stay behind --features sqlite" >&2
    exit 1
  fi

  echo "${label} carries no C storage engine"
}

assert_no_c_engine "made-embedded" -p made-embedded
assert_no_c_engine \
  "made-mcp embedded backend" \
  -p made-mcp \
  --no-default-features \
  --features embedded
