#!/usr/bin/env bash
set -euo pipefail

# Small, local gate for the C6 candidate. Expensive external campaigns remain
# explicit; this gate proves the tree is buildable and the reproducible
# evaluation contract is present.
cargo fmt --all -- --check
cargo check --workspace
cargo test -p made-console
cargo test -p made-app --lib
cargo test -p made-adapters --features sqlite
python3 scripts/evaluation/corte6_eval.py --print-cases >/dev/null
git diff --check
