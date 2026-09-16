#!/usr/bin/env python3
"""Contract for the draft/ready CI handover (plan §3.9, slices H1/H2/H5).

The handover is a rule spread over five files: dev-loop.yml runs only on
draft pull requests, quality-gate.yml and the container-backed and
packaging workflows stand down while a pull request is a draft and wake on
``ready_for_review``, a required ``gate`` context refuses to be green on a
draft, and ``scripts/ci/dev-loop.sh`` — what ``just dev`` runs — iterates
the same crates the workflow names.

Any one of those drifting silently turns "fast feedback" into "no gate".
This script is the regression test, and ``--self-test`` proves it still
catches the drift by mutating each rule and demanding a failure.

Run it:  python3 scripts/ci/dev-loop-workflow-contract.py [--self-test]
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

DEV_LOOP = ".github/workflows/dev-loop.yml"
QUALITY_GATE = ".github/workflows/quality-gate.yml"
INTEGRATION = ".github/workflows/integration.yml"
PLUGIN_PACKAGE = ".github/workflows/plugin-package.yml"
DEV_SCRIPT = "scripts/ci/dev-loop.sh"
JUSTFILE = "justfile"

SOURCES = (
    DEV_LOOP,
    QUALITY_GATE,
    INTEGRATION,
    PLUGIN_PACKAGE,
    DEV_SCRIPT,
    JUSTFILE,
)

# The dev loop answers drafts; everything else answers ready pull requests.
DRAFT_ONLY = (
    "if: github.event_name != 'pull_request' "
    "|| github.event.pull_request.draft == true"
)
READY_ONLY = (
    "if: github.event_name != 'pull_request' "
    "|| github.event.pull_request.draft == false"
)

# Each dev-loop lane and the stage of scripts/ci/dev-loop.sh it must run.
# dev-binary builds the artifact instead, so it names its own command.
DEV_LOOP_JOBS = {
    "dev-lint": "run: bash scripts/ci/dev-loop.sh lint",
    "dev-test": "run: bash scripts/ci/dev-loop.sh test",
    "dev-architecture": "run: bash scripts/ci/dev-loop.sh gates",
    "dev-binary": "run: cargo build --release -p made-mcp --locked",
}

# Every quality-gate job that must stand down with the `impact` job, and the
# planner output that routes it. Adding a job to the workflow without adding
# it here is caught by the `gate` needs-list check below, which compares
# against this same set.
QUALITY_GATE_JOBS = {
    "architecture": "architecture",
    "contract": "contract",
    "rustfmt": "rustfmt",
    "embedded-boundary": "embedded_boundary",
    "embedded-sqlite-gates": "embedded_sqlite",
    "clippy": "clippy",
    "test": "test",
    "coverage": "coverage",
    "container-image": "container",
    "helm-chart": "helm",
    "benches-compile": "benches",
    "publish-dry-run": "publish",
}

# Workflows that are part of the full gate and must not burn a runner on a
# draft: every job guarded, and ready_for_review in the trigger types.
STANDDOWN_WORKFLOWS = {
    INTEGRATION: ("integration-nats", "integration-postgres"),
    PLUGIN_PACKAGE: ("package",),
}

JOB_HEADER = re.compile(r"^  ([A-Za-z0-9_-]+):\s*$")
TOP_LEVEL = re.compile(r"^[A-Za-z_]")


def strip_comments(lines: list[str]) -> list[str]:
    return [line for line in lines if not line.lstrip().startswith("#")]


def job_block(text: str, name: str) -> str | None:
    lines = text.splitlines()
    start = next(
        (number for number, line in enumerate(lines) if line == f"  {name}:"),
        None,
    )
    if start is None:
        return None
    end = next(
        (
            number
            for number, line in enumerate(lines[start + 1 :], start + 1)
            if JOB_HEADER.fullmatch(line) or TOP_LEVEL.match(line)
        ),
        len(lines),
    )
    return "\n".join(strip_comments(lines[start:end]))


def trigger_block(text: str) -> str:
    lines = text.splitlines()
    start = next(
        (number for number, line in enumerate(lines) if line == "on:"),
        None,
    )
    if start is None:
        return ""
    end = next(
        (
            number
            for number, line in enumerate(lines[start + 1 :], start + 1)
            if TOP_LEVEL.match(line)
        ),
        len(lines),
    )
    return "\n".join(strip_comments(lines[start:end]))


def dev_packages_from_workflow(text: str) -> str | None:
    match = re.search(r"^  DEV_PACKAGES:[ \t]*(.+?)\s*$", text, re.MULTILINE)
    return match.group(1) if match else None


def dev_packages_from_script(text: str) -> str | None:
    match = re.search(
        r'^DEV_PACKAGES="\$\{DEV_PACKAGES:-(.+?)\}"\s*$', text, re.MULTILINE
    )
    return match.group(1) if match else None


def validate(sources: dict[str, str]) -> list[str]:
    failures: list[str] = []

    # --- H1: the dev loop runs on draft pull requests and nothing else ----
    dev_loop = sources[DEV_LOOP]
    triggers = trigger_block(dev_loop)
    if "pull_request:" not in triggers:
        failures.append("dev-loop lost its pull_request trigger")
    if "workflow_dispatch:" not in triggers:
        failures.append("dev-loop lost its workflow_dispatch trigger")
    if "push:" in triggers:
        failures.append(
            "dev-loop grew a push trigger: a merge to main must not start a "
            "development build"
        )
    if "cancel-in-progress: true" not in dev_loop:
        failures.append("dev-loop lost cancel-in-progress on its ref lane")

    for job, command in DEV_LOOP_JOBS.items():
        block = job_block(dev_loop, job)
        if block is None:
            failures.append(f"dev-loop lost the {job} job")
            continue
        if DRAFT_ONLY not in block:
            failures.append(f"dev-loop job {job} lost its draft-only guard")
        if command not in block:
            failures.append(f"dev-loop job {job} no longer runs: {command}")

    # --- H5: just dev and the workflow iterate the same crates ------------
    workflow_packages = dev_packages_from_workflow(dev_loop)
    script_packages = dev_packages_from_script(sources[DEV_SCRIPT])
    if workflow_packages is None:
        failures.append("dev-loop lost its DEV_PACKAGES env value")
    if script_packages is None:
        failures.append(f"{DEV_SCRIPT} lost its DEV_PACKAGES default")
    if (
        workflow_packages is not None
        and script_packages is not None
        and workflow_packages != script_packages
    ):
        failures.append(
            "DEV_PACKAGES drifted between the workflow and "
            f"{DEV_SCRIPT}: {workflow_packages!r} vs {script_packages!r}"
        )

    # --- H2: the full gate stands down on a draft and wakes on ready ------
    quality = sources[QUALITY_GATE]
    quality_triggers = trigger_block(quality)
    if "ready_for_review" not in quality_triggers:
        failures.append(
            "quality-gate lost the ready_for_review trigger type: the "
            "handover would wait for the next push"
        )

    impact = job_block(quality, "impact")
    if impact is None:
        failures.append("quality-gate lost its impact stand-down job")
    else:
        if "github.event.pull_request.draft == false" not in impact:
            failures.append("quality-gate impact job lost its draft stand-down guard")
        if "needs.tree-proof.outputs.skip != 'true'" not in impact:
            failures.append("quality-gate impact job no longer honours the tree proof")
        if "python3 scripts/ci/quality-gate-plan.py --self-test" not in impact:
            failures.append("quality-gate no longer proves its routing matrix first")
        if "--github-output" not in impact:
            failures.append("quality-gate impact job no longer publishes a plan")
        for gate in sorted(set(QUALITY_GATE_JOBS.values())):
            if f"      {gate}: ${{{{ steps.plan.outputs.{gate} }}}}" not in impact:
                failures.append(f"quality-gate impact job stopped exporting {gate}")

    proof = job_block(quality, "tree-proof")
    if proof is None:
        failures.append("quality-gate lost its tree-proof job")
    else:
        if "if: github.event_name == 'push'" not in proof:
            failures.append("tree-proof must only answer for pushes")
        if "run: bash scripts/ci/tree-already-proved.sh quality-gate.yml" not in proof:
            failures.append("tree-proof no longer runs the tree proof script")

    for job, gate in QUALITY_GATE_JOBS.items():
        block = job_block(quality, job)
        if block is None:
            failures.append(f"quality-gate lost the {job} job")
            continue
        needs = re.search(r"^    needs: \[(.+)\]\s*$", block, re.MULTILINE)
        if needs is None or "impact" not in [
            name.strip() for name in needs.group(1).split(",")
        ]:
            failures.append(
                f"quality-gate job {job} does not need impact, so it would "
                "run on a draft"
            )
        if f"needs.impact.outputs.{gate} == 'true'" not in block:
            failures.append(
                f"quality-gate job {job} is not routed by the planner output "
                f"{gate}"
            )
        if "!cancelled()" in block and "needs.impact.result == 'success'" not in block:
            failures.append(
                f"quality-gate job {job} opens with !cancelled() but does not "
                "require the planner to have succeeded, so it would run on a draft"
            )

    gate = job_block(quality, "gate")
    if gate is None:
        failures.append("quality-gate lost the required `gate` context")
    else:
        if "if: always()" not in gate:
            failures.append("gate must run always(), or a skip reports green")
        if 'if [[ "${DRAFT}" == "true" ]]; then' not in gate:
            failures.append("gate no longer fails on purpose on a draft")
        if 'if [[ "${TREE_PROVED}" == "true" ]]; then' not in gate:
            failures.append("gate no longer accepts an already-proved tree")
        missing = [
            job
            for job in ("tree-proof", "impact", *QUALITY_GATE_JOBS)
            if f"      - {job}\n" not in gate + "\n"
        ]
        if missing:
            failures.append(
                "gate does not wait for: " + ", ".join(missing)
            )

    # --- the rest of the full gate stands down too ------------------------
    for workflow, jobs in STANDDOWN_WORKFLOWS.items():
        text = sources[workflow]
        if "ready_for_review" not in trigger_block(text):
            failures.append(f"{workflow} lost the ready_for_review trigger type")
        for job in jobs:
            block = job_block(text, job)
            if block is None:
                failures.append(f"{workflow} lost the {job} job")
            elif READY_ONLY not in block:
                failures.append(f"{workflow} job {job} lost its draft stand-down guard")

    # --- H5: `just dev` is the same script, `just check` runs this --------
    justfile = sources[JUSTFILE]
    if "bash scripts/ci/dev-loop.sh {{STAGE}}" not in justfile:
        failures.append(
            "`just dev` no longer runs scripts/ci/dev-loop.sh, so the local "
            "loop and the workflow are two lists again"
        )
    if "python3 scripts/ci/quality-gate-plan.py --self-test" not in justfile:
        failures.append("`just workflow-contract` no longer proves the routing matrix")
    check = re.search(r"^check:(.*)$", justfile, re.MULTILINE)
    if check is None:
        failures.append("justfile lost its `check` recipe")
    elif "workflow-contract" not in check.group(1):
        failures.append("`just check` no longer proves this contract")

    return failures


MUTATIONS: dict[str, tuple[str, str, str]] = {
    "dev loop answers a ready pull request": (
        DEV_LOOP,
        f"    {DRAFT_ONLY}\n",
        "",
    ),
    "dev loop rebuilds every merge to main": (
        DEV_LOOP,
        "on:\n  pull_request:\n",
        "on:\n  push:\n    branches: [main]\n  pull_request:\n",
    ),
    "dev loop stops running the shared script": (
        DEV_LOOP,
        "run: bash scripts/ci/dev-loop.sh test",
        "run: cargo test --workspace",
    ),
    "just dev drifts from the workflow": (
        DEV_SCRIPT,
        'DEV_PACKAGES="${DEV_PACKAGES:--p made-core',
        'DEV_PACKAGES="${DEV_PACKAGES:--p made-mcp',
    ),
    "full gate never hears about ready for review": (
        QUALITY_GATE,
        "    types: [opened, synchronize, reopened, ready_for_review]\n",
        "",
    ),
    "full gate runs on drafts": (
        QUALITY_GATE,
        "      && (github.event_name != 'pull_request'"
        " || github.event.pull_request.draft == false)\n",
        "",
    ),
    "full gate ignores the tree proof": (
        QUALITY_GATE,
        "      needs.tree-proof.outputs.skip != 'true'\n",
        "      true\n",
    ),
    "the tree proof job disappears": (
        QUALITY_GATE,
        "  tree-proof:\n",
        "  tree-proof-disabled:\n",
    ),
    "the routing matrix stops proving itself": (
        QUALITY_GATE,
        "python3 scripts/ci/quality-gate-plan.py --self-test",
        "echo skipped",
    ),
    "a job stops being routed by the planner": (
        QUALITY_GATE,
        "    if: needs.impact.outputs.architecture == 'true'\n",
        "",
    ),
    "a routed job forgets the planner succeeded": (
        QUALITY_GATE,
        "      !cancelled() && needs.impact.result == 'success'\n"
        "      && needs.impact.outputs.clippy == 'true'\n",
        "      !cancelled()\n"
        "      && needs.impact.outputs.clippy == 'true'\n",
    ),
    "a gate job escapes the stand-down": (
        QUALITY_GATE,
        "  clippy:\n    runs-on: ubuntu-24.04\n    timeout-minutes: 20\n    needs: [impact, contract]\n",
        "  clippy:\n    runs-on: ubuntu-24.04\n    timeout-minutes: 20\n    needs: [contract]\n",
    ),
    "the required context goes green on a draft": (
        QUALITY_GATE,
        'if [[ "${DRAFT}" == "true" ]]; then',
        'if [[ "${DRAFT}" == "never" ]]; then',
    ),
    "the required context stops waiting for coverage": (
        QUALITY_GATE,
        "      - coverage\n",
        "",
    ),
    "the packaging matrix burns runners on drafts": (
        PLUGIN_PACKAGE,
        f"    {READY_ONLY}\n",
        "",
    ),
    "just dev stops running the shared script": (
        JUSTFILE,
        "bash scripts/ci/dev-loop.sh {{STAGE}}",
        "cargo test --workspace",
    ),
    "just stops proving the routing matrix": (
        JUSTFILE,
        "    python3 scripts/ci/quality-gate-plan.py --self-test\n",
        "",
    ),
    "just check stops proving this contract": (
        JUSTFILE,
        "check: workflow-contract ",
        "check: ",
    ),
    "the container suites burn runners on drafts": (
        INTEGRATION,
        f"    {READY_ONLY}\n",
        "",
    ),
}


def self_test(sources: dict[str, str]) -> None:
    for name, (path, old, new) in MUTATIONS.items():
        if old not in sources[path]:
            raise SystemExit(
                f"dev-loop workflow contract self-test could not apply: {name} "
                f"(pattern absent from {path})"
            )
        mutated = dict(sources)
        mutated[path] = sources[path].replace(old, new, 1)
        if not validate(mutated):
            raise SystemExit(
                f"dev-loop workflow contract missed mutation: {name}"
            )
    print(f"self-test: {len(MUTATIONS)} mutation guards hold")


def main() -> int:
    sources = {path: (ROOT / path).read_text(encoding="utf-8") for path in SOURCES}

    failures = validate(sources)
    if failures:
        print(
            "dev-loop workflow contract failed:\n  - " + "\n  - ".join(failures),
            file=sys.stderr,
        )
        return 1

    if "--self-test" in sys.argv[1:]:
        self_test(sources)

    print(
        "dev-loop workflow contract passed: "
        f"{len(DEV_LOOP_JOBS)} draft-only lanes, "
        f"{len(QUALITY_GATE_JOBS)} routed gate jobs, "
        "one required gate context, DEV_PACKAGES agreed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
