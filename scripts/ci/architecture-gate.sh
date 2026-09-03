#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

# Hexagonal/DDD architecture ratchet. The migration is intentionally
# incremental: current structural debt is explicit in a checked-in baseline,
# can only shrink, and can never grow silently.
#
# Refresh after paying debt down:
#   MADE_ARCHITECTURE_BASELINE=write bash scripts/ci/architecture-gate.sh

python3 - <<'PY'
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

root = Path.cwd().resolve()
baseline_path = root / "docs/architecture/conformance.tsv"
monolith_lines = 600

primary_type = re.compile(
    r"^\s*pub(?:\([^)]*\))?\s+(?:struct|enum|trait|union|type)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
primitive_field = re.compile(
    r"^\s*pub\s+[A-Za-z_][A-Za-z0-9_]*\s*:\s*(?:String|bool|[ui](?:8|16|32|64|128|size)|f(?:32|64))\b"
)


def run(*args: str) -> str:
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


def workspace_packages() -> dict[str, dict[str, object]]:
    metadata = json.loads(run("cargo", "metadata", "--no-deps", "--format-version", "1"))
    return {package["name"]: package for package in metadata["packages"]}


# These are the allowed inward dependencies between MADE crates. Test drivers
# are deliberately outside this table: they may depend on every production
# ring in order to prove the assembled system.
allowed_internal = {
    "made-core": set(),
    "made-api": set(),
    "made-app": {"made-core"},
    "made-proto": set(),
    "made-mcp-proto": set(),
    "made-adapters": {"made-core", "made-app", "made-proto"},
    "made-embedded": {"made-api", "made-core", "made-app", "made-adapters"},
    "made": {"made-core", "made-app", "made-adapters", "made-proto"},
    "made-mcp": {
        "made-core",
        "made-app",
        "made-adapters",
        "made-embedded",
        "made-mcp-proto",
    },
}

packages = workspace_packages()
package_names = set(packages)
failures: list[str] = []
for package_name, allowed in allowed_internal.items():
    package = packages[package_name]
    actual = {
        dependency["name"]
        for dependency in package["dependencies"]
        if dependency["name"] in package_names and dependency.get("kind") != "dev"
    }
    forbidden = sorted(actual - allowed)
    if forbidden:
        failures.append(
            f"{package_name}: outward workspace dependencies violate the hexagon: "
            + ", ".join(forbidden)
        )

# The domain may use serialization and deterministic domain utilities, but it
# must never acquire deployment, transport, filesystem, database or vendor SDKs.
forbidden_core_dependencies = {
    "anyhow",
    "async-nats",
    "axum",
    "figment",
    "opentelemetry",
    "prost",
    "redb",
    "reqwest",
    "rusqlite",
    "sqlx",
    "tokio",
    "tonic",
    "tracing-opentelemetry",
}
core_dependencies = {
    dependency["name"]
    for dependency in packages["made-core"]["dependencies"]
    if dependency.get("kind") != "dev"
}
forbidden = sorted(core_dependencies & forbidden_core_dependencies)
if forbidden:
    failures.append("made-core: infrastructure dependency leaked into the domain: " + ", ".join(forbidden))


def tracked_sources() -> list[Path]:
    listed = run("git", "ls-files", "crates").splitlines()
    tracked = {
        root / name
        for name in listed
        if "/src/" in name and name.endswith(".rs") and (root / name).exists()
    }
    present = set((root / "crates").glob("*/src/**/*.rs"))
    return sorted(tracked | present)


def relative(path: Path) -> str:
    return path.relative_to(root).as_posix()


def production_line_count(lines: list[str]) -> int:
    """Exclude a conventional trailing unit-test module from monolith size."""
    for index, line in enumerate(lines):
        if line.strip() != "#[cfg(test)]":
            continue
        following = [
            candidate.strip()
            for candidate in lines[index + 1 : index + 4]
            if candidate.strip()
        ]
        if following and following[0] == "mod tests {":
            return index
    return len(lines)


debt: dict[str, str] = {}
for source in tracked_sources():
    lines = source.read_text(encoding="utf-8").splitlines()
    source_name = relative(source)
    types = [match.group(1) for line in lines if (match := primary_type.match(line))]
    reasons: list[str] = []
    if len(types) > 1:
        reasons.append(f"types={len(types)}")
    production_lines = production_line_count(lines)
    line_budget_exempt = source_name.startswith(
        (
            "crates/made-consumer-smoke/",
            "crates/made-e2e-runner/",
            "crates/made-tests-integration/",
        )
    ) or source.name.endswith("_test_support.rs")
    if not line_budget_exempt and production_lines > monolith_lines:
        reasons.append(f"lines={production_lines}")
    if source.is_relative_to(root / "crates/made-core/src"):
        primitives = sum(1 for line in lines if primitive_field.match(line))
        if primitives:
            reasons.append(f"primitive_fields={primitives}")
    if reasons:
        debt[source_name] = ",".join(reasons)

if os.environ.get("MADE_ARCHITECTURE_BASELINE") == "write":
    baseline_path.parent.mkdir(parents=True, exist_ok=True)
    with baseline_path.open("w", encoding="utf-8") as baseline:
        baseline.write("# MADE architecture debt. Values may shrink and must never grow silently.\n")
        baseline.write(
            f"# Budget: one primary type per source file; at most {monolith_lines} "
            "production lines (test drivers/support exempt from size only).\n"
        )
        baseline.write("# Public primitive fields are additionally counted in made-core.\n")
        baseline.write("path\tdebt\n")
        for name, measures in sorted(debt.items()):
            baseline.write(f"{name}\t{measures}\n")
    print(f"wrote {baseline_path.relative_to(root)} with {len(debt)} debt entries")
    sys.exit(0)

if not baseline_path.exists():
    sys.exit(
        f"missing {baseline_path.relative_to(root)}; initialize it with "
        "MADE_ARCHITECTURE_BASELINE=write bash scripts/ci/architecture-gate.sh"
    )

baseline: dict[str, str] = {}
for line in baseline_path.read_text(encoding="utf-8").splitlines():
    if not line or line.startswith("#") or line.startswith("path\t"):
        continue
    name, _, measures = line.partition("\t")
    baseline[name] = measures

for name, measures in sorted(debt.items()):
    if name not in baseline:
        failures.append(f"{name}: new architecture debt ({measures})")
        continue
    before = dict(part.split("=") for part in baseline[name].split(","))
    now = dict(part.split("=") for part in measures.split(","))
    for measure, value in now.items():
        if int(value) > int(before.get(measure, 0)):
            failures.append(
                f"{name}: {measure} grew from {before.get(measure, 0)} to {value}"
            )

paid = sorted(set(baseline) - set(debt))
print(f"architecture gate: {len(tracked_sources())} production sources")
print(f"  debt carried: {len(debt)} of {len(baseline)} baselined files")
print(f"  debt paid:    {len(paid)}")
if paid:
    print("  refresh the baseline after this change:")
    print("    MADE_ARCHITECTURE_BASELINE=write bash scripts/ci/architecture-gate.sh")

if failures:
    for failure in failures:
        print(f"  {failure}", file=sys.stderr)
    sys.exit(1)
PY
