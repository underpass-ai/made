#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

: "${COVERAGE_MIN:=80}"

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

COVERAGE_REPORT="$(python3 - "${SUMMARY_JSON}" <<'PY'
import json
import pathlib
import sys

production = {
    "made-core",
    "made-api",
    "made-app",
    "made-adapters",
    "made-embedded",
    "made",
    "made-mcp",
}

with open(sys.argv[1], encoding="utf-8") as summary_file:
    files = json.load(summary_file)["data"][0]["files"]

totals = {crate: [0, 0] for crate in production}
for file in files:
    parts = pathlib.Path(file["filename"]).parts
    try:
        crate = parts[parts.index("crates") + 1]
    except (ValueError, IndexError):
        continue
    if crate not in production:
        continue
    lines = file["summary"]["lines"]
    totals[crate][0] += lines["covered"]
    totals[crate][1] += lines["count"]

covered = sum(value[0] for value in totals.values())
count = sum(value[1] for value in totals.values())
if count == 0:
    raise SystemExit("coverage gate found no production source lines")

print(f"total\t{covered}\t{count}\t{100 * covered / count:.2f}")
for crate in sorted(totals):
    crate_covered, crate_count = totals[crate]
    percent = 100 * crate_covered / crate_count if crate_count else 100.0
    print(f"{crate}\t{crate_covered}\t{crate_count}\t{percent:.2f}")
PY
)"

COVERAGE_PCT="$(awk -F '\t' '$1 == "total" { print $4 }' <<<"${COVERAGE_REPORT}")"

echo ">>> production coverage by crate (covered / lines / percent)"
awk -F '\t' '$1 != "total" { printf "    %-22s %6d / %-6d %6.2f%%\n", $1, $2, $3, $4 }' \
  <<<"${COVERAGE_REPORT}"
echo ">>> production coverage (lines): ${COVERAGE_PCT}% (minimum ${COVERAGE_MIN}%)"

python3 - "${COVERAGE_PCT}" "${COVERAGE_MIN}" <<'PY'
import sys
pct = float(sys.argv[1])
threshold = float(sys.argv[2])
if pct + 1e-9 < threshold:
    sys.stderr.write(f"coverage gate failed: {pct}% < {threshold}%\n")
    sys.exit(1)
PY
