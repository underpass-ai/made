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

# SQLite is the canonical embedded store. The dependency boundary must include
# it and must never regain the retired Redb engine.
assert_canonical_sqlite() {
  local label="$1"
  shift
  local tree

  tree="$(cargo tree --locked "$@" -e normal --prefix none --format '{p}')"
  if ! grep -qE '^rusqlite ' <<<"${tree}"; then
    echo "${label} does not carry the canonical SQLite store" >&2
    exit 1
  fi
  if grep -qE '^redb ' <<<"${tree}"; then
    echo "${label} still carries the retired Redb engine" >&2
    exit 1
  fi

  echo "${label} carries SQLite and no Redb"
}

assert_canonical_sqlite "made-embedded" -p made-embedded
assert_canonical_sqlite \
  "made-mcp embedded backend" \
  -p made-mcp \
  --no-default-features \
  --features embedded
