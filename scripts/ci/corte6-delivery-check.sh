#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

test -z "$(git status --porcelain)" || {
  echo "working tree must be clean before the C6 delivery check" >&2
  exit 1
}
cargo run -p made-console -- --help >/dev/null
python3 scripts/evaluation/corte6_eval.py --print-cases >/dev/null
test -f docs/corte6/c6-13-guia-mcp-local.md
test -x scripts/ci/corte6-integration-gate.sh
test -x scripts/ci/corte6-scenarios.sh
printf '%s\n' "C6 local delivery contract is present at $(git rev-parse HEAD)"
