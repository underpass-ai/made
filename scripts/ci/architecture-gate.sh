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

python3 - "$@" <<'PY'
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
zero_type_lines = 400

primary_type = re.compile(
    r"^\s*(?:(?:(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum|trait|union)\s+"
    r"([A-Za-z_][A-Za-z0-9_]*))|(?:pub(?:\([^)]*\))?\s+type\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)\s*=))"
)
public_primitive_field = re.compile(
    r"^    pub\s+[A-Za-z_][A-Za-z0-9_]*\s*:\s*"
    r"(?:Option\s*<\s*)?(?:String|bool|[ui](?:8|16|32|64|128|size)|f(?:32|64))\b"
)
boundary_primitive_field = re.compile(
    r"^    (?:pub(?:\([^)]*\))?\s+)?[A-Za-z_][A-Za-z0-9_]*\s*:\s*"
    r"(?:Option\s*<\s*)?(?:String|bool|[ui](?:8|16|32|64|128|size)|f(?:32|64))\b"
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


def test_only_module_sources(sources: list[Path]) -> set[Path]:
    """Resolve external modules that their parent compiles only under test."""
    test_only: set[Path] = set()
    external_module = re.compile(
        r"^(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;\s*$"
    )
    for source in sources:
        lines = source.read_text(encoding="utf-8").splitlines()
        for index, line in enumerate(lines):
            if line.strip() != "#[cfg(test)]":
                continue
            following = (
                candidate.strip()
                for candidate in lines[index + 1 :]
                if candidate.strip()
                and not candidate.lstrip().startswith(("//", "#["))
            )
            declaration = next(following, "")
            match = external_module.match(declaration)
            if not match:
                continue
            module_root = (
                source.parent
                if source.stem in {"lib", "main", "mod"}
                else source.parent / source.stem
            )
            candidates = (
                module_root / f"{match.group(1)}.rs",
                module_root / match.group(1) / "mod.rs",
            )
            test_only.update(candidate for candidate in candidates if candidate in sources)
    return test_only


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


def module_primary_types(lines: list[str]) -> list[str]:
    """Return primary types declared in file or inline-module item scope.

    Rust permits module items to be indented inside ``mod name { ... }``. A
    plain column-zero regex misses those, while an unrestricted whitespace
    regex also counts function-local and test-only helper types. Rustfmt keeps
    item-opening braces on the declaration line, so an indentation stack is
    sufficient to distinguish those scopes without trying to parse Rust.
    """
    contexts: list[tuple[int, str]] = []
    pending_test_at: int | None = None
    types: list[str] = []
    module_item = re.compile(
        r"^(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?mod\s+"
        r"[A-Za-z_][A-Za-z0-9_]*\s*\{\s*$"
    )

    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue
        indent = len(line) - len(line.lstrip())
        while contexts and indent <= contexts[-1][0]:
            contexts.pop()

        if stripped == "#[cfg(test)]":
            pending_test_at = indent
            continue
        if stripped.startswith("#["):
            continue

        in_production_module = all(kind == "module" for _, kind in contexts)
        if in_production_module and (match := primary_type.match(line)):
            types.append(match.group(1) or match.group(2))

        if module_item.match(stripped):
            kind = "test" if pending_test_at == indent else "module"
            contexts.append((indent, kind))
        elif stripped.endswith("{"):
            # Functions, impls, traits, macros and const blocks are not module
            # item scope. One enclosing marker is enough to exclude any types
            # nested inside them until indentation returns to this level.
            contexts.append((indent, "other"))
        pending_test_at = None
    return types


def source_debt(source: Path, lines: list[str]) -> str | None:
    source_name = relative(source)
    production_lines = production_line_count(lines)
    production = lines[:production_lines]
    types = module_primary_types(production)
    reasons: list[str] = []
    if len(types) > 1:
        reasons.append(f"types={len(types)}")
    line_budget_exempt = source_name.startswith(
        (
            "crates/made-consumer-smoke/",
            "crates/made-e2e-runner/",
            "crates/made-tests-integration/",
        )
    ) or source.name.endswith("_test_support.rs")
    if not line_budget_exempt and production_lines > monolith_lines:
        reasons.append(f"lines={production_lines}")
    if not line_budget_exempt and not types and production_lines > zero_type_lines:
        reasons.append(f"zero_type_lines={production_lines}")
    app_boundary = (
        source.parent == root / "crates/made-app/src/usecases"
        and source.stem.endswith(("_input", "_output", "_document", "_stage", "_summary"))
    )
    if source.is_relative_to(root / "crates/made-core/src"):
        primitives = sum(1 for line in production if public_primitive_field.match(line))
        if primitives:
            reasons.append(f"primitive_fields={primitives}")
    elif app_boundary:
        primitives = sum(1 for line in production if boundary_primitive_field.match(line))
        if primitives:
            reasons.append(f"primitive_fields={primitives}")
    return ",".join(reasons) or None


def architecture_self_test() -> None:
    inline_fixture = [
        "mod fixture {",
        "    struct First;",
        "    enum Second {}",
        "}",
        "fn helper() {",
        "    struct FunctionLocal;",
        "}",
        "#[cfg(test)]",
        "mod tests {",
        "    struct TestOnly;",
        "}",
    ]
    inline_debt = source_debt(root / "crates/made-core/src/fixture.rs", inline_fixture)
    if inline_debt != "types=2":
        raise AssertionError(
            f"indented private primary fixture: expected types=2, got {inline_debt!r}"
        )

    option_fixture = [
        "struct FixtureInput {",
        "    limit: Option<u64>,",
        "}",
    ]
    option_debt = source_debt(
        root / "crates/made-app/src/usecases/fixture_input.rs", option_fixture
    )
    if option_debt != "primitive_fields=1":
        raise AssertionError(
            "application Option primitive fixture: expected primitive_fields=1, "
            f"got {option_debt!r}"
        )

    zero_type_fixture = ["// fixture line"] * (zero_type_lines + 1)
    zero_type_debt = source_debt(
        root / "crates/made-mcp/src/zero_type_fixture.rs", zero_type_fixture
    )
    expected = f"zero_type_lines={zero_type_lines + 1}"
    if zero_type_debt != expected:
        raise AssertionError(
            f"zero-primary-type fixture: expected {expected}, got {zero_type_debt!r}"
        )


architecture_self_test()
if "--self-test" in sys.argv[1:]:
    print("architecture gate self-test passed: 3 regression fixtures")
    sys.exit(0)
if sys.argv[1:]:
    sys.exit(f"unknown architecture gate argument: {' '.join(sys.argv[1:])}")


sources = tracked_sources()
production_sources = [source for source in sources if source not in test_only_module_sources(sources)]
debt: dict[str, str] = {}
for source in production_sources:
    lines = source.read_text(encoding="utf-8").splitlines()
    source_name = relative(source)
    if measures := source_debt(source, lines):
        debt[source_name] = measures

if os.environ.get("MADE_ARCHITECTURE_BASELINE") == "write":
    baseline_path.parent.mkdir(parents=True, exist_ok=True)
    with baseline_path.open("w", encoding="utf-8") as baseline:
        baseline.write("# MADE architecture debt. Values may shrink and must never grow silently.\n")
        baseline.write(
            f"# Budget: one primary type per source file; at most {monolith_lines} "
            "production lines (test drivers/support exempt from size only).\n"
        )
        baseline.write(
            f"# Zero-primary-type production files have a lower {zero_type_lines}-line budget.\n"
        )
        baseline.write(
            "# Primitive fields are counted in made-core and made-app use-case boundary types.\n"
        )
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
print(f"architecture gate: {len(production_sources)} production sources")
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
