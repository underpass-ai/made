#!/usr/bin/env python3
"""Fail closed on per-crate coverage and reductions to committed floors."""
from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
FLOORS = "docs/architecture/coverage-floors.tsv"
PRODUCTION = {"made-core", "made-api", "made-app", "made-adapters", "made-embedded", "made", "made-mcp"}


def default_baseline_ref(environment: dict[str, str]) -> str:
    if (environment.get("GITHUB_EVENT_NAME") == "push"
            and environment.get("GITHUB_REF") == "refs/heads/main"):
        return "HEAD^"
    return "origin/" + (environment.get("GITHUB_BASE_REF") or "main")


def read_floors(text: str) -> dict[str, float]:
    floors: dict[str, float] = {}
    for line in text.splitlines():
        if not line or line.startswith("#") or line == "crate\tminimum_percent":
            continue
        crate, percent = line.split("\t")
        value = float(percent)
        if crate in floors or not math.isfinite(value) or not 80 <= value <= 100:
            raise ValueError(f"invalid coverage floor: {line}")
        floors[crate] = value
    if set(floors) != PRODUCTION:
        raise ValueError("coverage floors must name every production crate exactly once")
    return floors


def check_ratchet(current: dict[str, float], previous: dict[str, float]) -> None:
    for crate, minimum in previous.items():
        if current.get(crate, 0) < minimum:
            raise ValueError(f"{crate}: floor decreased from {minimum} to {current.get(crate)}")


def baseline_floors(ref: str) -> dict[str, float]:
    # Distinguish a first introduction from missing Git history or a broken ref.
    subprocess.run(["git", "rev-parse", "--verify", f"{ref}^{{commit}}"], cwd=ROOT,
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    listed = subprocess.run(["git", "ls-tree", "--name-only", ref, "--", FLOORS],
                            cwd=ROOT, check=True, capture_output=True, text=True)
    if not listed.stdout.strip():
        return {}
    prior = subprocess.run(["git", "show", f"{ref}:{FLOORS}"], cwd=ROOT,
                           check=True, capture_output=True, text=True)
    return read_floors(prior.stdout)


def coverage_totals(summary: dict) -> dict[str, tuple[int, int]]:
    totals = {crate: (0, 0) for crate in PRODUCTION}
    for file in summary["data"][0]["files"]:
        parts = Path(file["filename"]).parts
        try:
            crate = parts[parts.index("crates") + 1]
        except (ValueError, IndexError):
            continue
        if crate not in PRODUCTION:
            continue
        lines = file["summary"]["lines"]
        covered, count = lines["covered"], lines["count"]
        if type(covered) is not int or type(count) is not int or not 0 <= covered <= count:
            raise ValueError(f"{crate}: invalid line coverage counts")
        previous_covered, previous_count = totals[crate]
        totals[crate] = previous_covered + covered, previous_count + count
    return totals


def check_coverage(totals: dict[str, tuple[int, int]], floors: dict[str, float], minimum: float) -> list[str]:
    if not math.isfinite(minimum) or not 80 <= minimum <= 100:
        raise ValueError("production coverage minimum must be between 80 and 100")
    failures = []
    for crate in sorted(PRODUCTION):
        covered, count = totals[crate]
        if count == 0:
            failures.append(f"{crate}: no production coverage data")
        elif covered * 100 < floors[crate] * count:
            failures.append(f"{crate}: {100 * covered / count:.4f}% < floor {floors[crate]}%")
    covered = sum(value[0] for value in totals.values())
    count = sum(value[1] for value in totals.values())
    if count == 0 or covered * 100 < minimum * count:
        failures.append(f"production total: {covered}/{count} below {minimum}%")
    return failures


def self_test() -> None:
    floor_text = "\n".join(f"{crate}\t80" for crate in sorted(PRODUCTION))
    floors = read_floors(floor_text)
    totals = {crate: (80, 100) for crate in PRODUCTION}
    assert not check_coverage(totals, floors, 80)
    assert check_coverage(totals | {"made": (79999, 100000)}, floors, 80)
    assert check_coverage(totals | {"made": (0, 0)}, floors, 80)
    assert check_coverage(totals, floors, 81)
    check_ratchet(floors | {"made": 81}, floors)
    bad_calls = [
        lambda: check_ratchet(floors, floors | {"made": 81}),
        lambda: read_floors(floor_text + "\nmade\t80"),
        lambda: read_floors(floor_text.replace("made\t80", "made\t79")),
        lambda: read_floors(floor_text.replace("made\t80", "made\tnan")),
        lambda: read_floors("made\t80"),
        lambda: check_coverage(totals, floors, 79),
    ]
    for check in bad_calls:
        try:
            check()
        except ValueError:
            pass
        else:
            raise AssertionError("invalid coverage policy was accepted")
    files = [{"filename": f"/workspace/crates/{crate}/src/lib.rs",
              "summary": {"lines": {"covered": 80, "count": 100}}} for crate in PRODUCTION]
    files.append({"filename": "/workspace/crates/made-proto/src/generated.rs",
                  "summary": {"lines": {"covered": 0, "count": 999999}}})
    assert coverage_totals({"data": [{"files": files}]}) == totals
    assert default_baseline_ref({}) == "origin/main"
    assert default_baseline_ref({"GITHUB_BASE_REF": ""}) == "origin/main"
    assert default_baseline_ref({"GITHUB_BASE_REF": "release"}) == "origin/release"
    assert default_baseline_ref({
        "GITHUB_EVENT_NAME": "push", "GITHUB_REF": "refs/heads/main"
    }) == "HEAD^"
    print(
        "coverage floor self-test passed: boundaries, missing data, ratchet, "
        "invalid policy, exclusions, baseline refs"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("summary", nargs="?", type=Path)
    parser.add_argument("--minimum", type=float, default=80)
    parser.add_argument("--baseline-ref", default=default_baseline_ref(os.environ))
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.summary is None:
        parser.error("summary is required unless --self-test is used")
    floors = read_floors((ROOT / FLOORS).read_text())
    check_ratchet(floors, baseline_floors(args.baseline_ref))
    totals = coverage_totals(json.loads(args.summary.read_text()))
    for crate in sorted(PRODUCTION):
        covered, count = totals[crate]
        percent = f"{100 * covered / count:.4f}%" if count else "missing"
        print(f"{crate:22} {covered:6}/{count:<6} {percent:>10} floor {floors[crate]:g}%")
    failures = check_coverage(totals, floors, args.minimum)
    if failures:
        raise SystemExit("coverage gate failed:\n" + "\n".join(failures))
    print("coverage gate passed: production total and every crate meet their floors")


if __name__ == "__main__":
    main()
