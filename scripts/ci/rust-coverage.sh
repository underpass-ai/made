#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

: "${COVERAGE_MIN:=80}"

python3 scripts/ci/coverage-floors.py --self-test

mkdir -p target/llvm-cov

cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --locked --no-report
cargo llvm-cov report --locked --lcov --output-path target/llvm-cov/lcov.info

# Enforce minimum unit coverage over production code. Test drivers and
# generated transport crates are deliberately excluded from the denominator:
# their purpose is to exercise the product, not to make its percentage larger
# or smaller. The gate is parsed from the JSON summary so CI fails closed.
SUMMARY_JSON="target/llvm-cov/summary.json"
cargo llvm-cov report --locked --json --summary-only --output-path "${SUMMARY_JSON}"

python3 scripts/ci/coverage-floors.py "${SUMMARY_JSON}" --minimum "${COVERAGE_MIN}"
