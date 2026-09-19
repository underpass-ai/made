#!/usr/bin/env python3
"""Small, dependency-free runner for the C6 reproducible evaluation contract.

The runner intentionally treats a provider as an opaque command. It records
the exact fixture, command, versions supplied by the caller, bounded output
and deterministic hashes. Missing usage/cost is represented as ``unknown``;
it is never coerced to zero.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shlex
import subprocess
import sys
import time
from pathlib import Path


MAX_OUTPUT = 16_384


def parse_spec(path: Path) -> dict:
    # The committed spec deliberately uses a tiny TOML subset so this script
    # remains runnable on a clean machine with Python's standard library only.
    version = None
    seed = None
    families: list[dict] = []
    current: dict | None = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("version ="):
            version = line.split("=", 1)[1].strip().strip('"')
        elif line.startswith("seed ="):
            seed = line.split("=", 1)[1].strip().strip('"')
        elif line.startswith("{ name ="):
            body = line.strip("{}, ")
            values: dict[str, str | int] = {}
            for part in body.split(","):
                key, value = (piece.strip() for piece in part.split("=", 1))
                values[key] = int(value) if value.isdigit() else value.strip('"')
            families.append(values)  # type: ignore[arg-type]
    if not version or not seed or not families:
        raise ValueError(f"invalid evaluation spec: {path}")
    if sum(int(item["count"]) for item in families) < 100:
        raise ValueError("C6 corpus must contain at least 100 cases")
    names = [str(item["name"]) for item in families]
    if len(names) != len(set(names)):
        raise ValueError("C6 corpus family names must be unique")
    return {"version": version, "seed": seed, "families": families}


def fixture(spec: dict, family: dict, ordinal: int) -> dict:
    family_name = str(family["name"])
    case_id = f"{family_name}-{ordinal:03d}"
    prompt = (
        f"MADE C6 fixture {case_id}: produce a bounded, auditable result for "
        f"the {family_name} family. Do not invent external effects."
    )
    return {
        "id": case_id,
        "family": family_name,
        "ordinal": ordinal,
        "input": {"prompt": prompt, "seed": spec["seed"]},
        "rubric": {"oracle": family["oracle"], "expected": "valid_json"},
    }


def cases(spec: dict) -> list[dict]:
    result: list[dict] = []
    for family in spec["families"]:
        result.extend(fixture(spec, family, ordinal) for ordinal in range(1, int(family["count"]) + 1))
    return result


def digest(value: object) -> str:
    payload = json.dumps(value, sort_keys=True, ensure_ascii=False).encode()
    return hashlib.sha256(payload).hexdigest()


def run_case(command: list[str], case: dict, timeout: float) -> dict:
    started = time.monotonic_ns()
    try:
        completed = subprocess.run(
            command,
            input=json.dumps(case, sort_keys=True) + "\n",
            text=True,
            capture_output=True,
            timeout=timeout,
            check=False,
            env={**os.environ, "MADE_EVAL_CASE_ID": case["id"]},
        )
        outcome = "completed" if completed.returncode == 0 else "failed"
        stdout = completed.stdout[:MAX_OUTPUT]
        stderr = completed.stderr[:MAX_OUTPUT]
        return {
            "id": case["id"],
            "family": case["family"],
            "outcome": outcome,
            "exit_code": completed.returncode,
            "elapsed_ms": (time.monotonic_ns() - started) / 1_000_000,
            "input_sha256": digest(case["input"]),
            "stdout_sha256": digest(stdout),
            "stdout": stdout,
            "stderr": stderr,
            "usage": {"tokens": "unknown", "cost": "unknown", "source": "not_reported"},
        }
    except subprocess.TimeoutExpired as error:
        return {
            "id": case["id"],
            "family": case["family"],
            "outcome": "timeout",
            "exit_code": None,
            "elapsed_ms": (time.monotonic_ns() - started) / 1_000_000,
            "input_sha256": digest(case["input"]),
            "stdout_sha256": digest((error.stdout or "")[:MAX_OUTPUT]),
            "stdout": (error.stdout or "")[:MAX_OUTPUT],
            "stderr": (error.stderr or "")[:MAX_OUTPUT],
            "usage": {"tokens": "unknown", "cost": "unknown", "source": "not_reported"},
        }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", type=Path, default=Path(__file__).with_name("corte6-corpus.toml"))
    parser.add_argument("--command", help="provider command; receives one JSON fixture on stdin")
    parser.add_argument("--output", type=Path, required=False)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--print-cases", action="store_true")
    args = parser.parse_args()
    if args.repeat < 1:
        parser.error("--repeat must be positive")
    try:
        spec = parse_spec(args.spec)
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 2
    expanded = cases(spec)
    if args.print_cases:
        print(json.dumps({"spec": spec, "cases": expanded}, indent=2, sort_keys=True))
        return 0
    if not args.command:
        parser.error("--command is required unless --print-cases is used")
    command = shlex.split(args.command)
    results = []
    for repetition in range(1, args.repeat + 1):
        for case in expanded:
            result = run_case(command, case, args.timeout)
            result["repetition"] = repetition
            results.append(result)
    report = {
        "contract": "made.c6.reproducible-evaluation.v1",
        "spec": spec,
        "case_count": len(expanded),
        "repetitions": args.repeat,
        "command": command,
        "results": results,
    }
    encoded = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded + "\n", encoding="utf-8")
    else:
        print(encoded)
    return 0 if all(item["outcome"] == "completed" for item in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
