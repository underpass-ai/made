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

Nothing here is a second copy of the workflow. The job list, the planner
output each job is routed by, the ``gate`` job's needs-list and the tree
proof's idea of the full matrix are all *derived* from the files they
describe and then cross-checked against each other, because a contract
that reads its own hard-coded table on both sides of a comparison cannot
see a job that was never added to it.

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
DEPENDENCY_REVIEW = ".github/workflows/dependency-review.yml"
PUBLISH_DISTRIBUTION = ".github/workflows/publish-distribution.yml"
DEV_SCRIPT = "scripts/ci/dev-loop.sh"
JUSTFILE = "justfile"
PLANNER = "scripts/ci/quality-gate-plan.py"
TREE_PROOF = "scripts/ci/tree-already-proved.sh"

SOURCES = (
    DEV_LOOP,
    QUALITY_GATE,
    INTEGRATION,
    PLUGIN_PACKAGE,
    DEPENDENCY_REVIEW,
    PUBLISH_DISTRIBUTION,
    DEV_SCRIPT,
    JUSTFILE,
    PLANNER,
    TREE_PROOF,
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

# The three quality-gate jobs that are not routed gates: the proof, the
# planner, and the required context. Every *other* job in the workflow must
# need `impact` and be routed by one of its outputs — that rule is what
# catches a job added to the workflow and to nobody's list.
UNROUTED_JOBS = ("tree-proof", "impact", "gate")

# The four moments a workflow that stands down on drafts has to hear about.
# Shrink the list and the handover waits for the next push instead of
# happening on `gh pr ready`.
REQUIRED_TRIGGER_TYPES = ("opened", "synchronize", "reopened", "ready_for_review")

# What each Rust gate proves. The planner deliberately exports no crate
# list (see `ci(planner): drop cargo_packages rather than wire it`): the
# closure decides which gates run, `--workspace` decides what they build.
WORKSPACE_COMMANDS = {
    "clippy": "cargo clippy --workspace",
    "test": "cargo test --workspace",
}

# Workflows that are part of the full gate and must not burn a runner on a
# draft: every job guarded, and ready_for_review in the trigger types.
STANDDOWN_WORKFLOWS = {
    INTEGRATION: ("integration-nats", "integration-postgres"),
    PLUGIN_PACKAGE: ("package",),
}

# A pull-request job holds a token the pull request's own scripts can
# spend, so the packaging matrix reads and the tag-gated release upload is
# the only job in that workflow that writes.
PACKAGING_JOB = "package"
RELEASE_JOB = "release-upload"

ACTION_REFERENCE = re.compile(r"^\s*uses:\s*(\S+)\s*$", re.MULTILINE)
COMMIT_SHA = re.compile(r"[0-9a-f]{40}")

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


def workflow_jobs(text: str) -> list[str]:
    """Every job name in the workflow's `jobs:` block, in file order."""
    lines = text.splitlines()
    start = next((number for number, line in enumerate(lines) if line == "jobs:"), None)
    if start is None:
        return []
    names: list[str] = []
    for line in lines[start + 1 :]:
        if TOP_LEVEL.match(line):
            break
        if line.lstrip().startswith("#"):
            continue
        header = JOB_HEADER.fullmatch(line)
        if header:
            names.append(header.group(1))
    return names


def needs_list(block: str) -> list[str]:
    """The jobs a `needs:` key names, whether inline or as a block list."""
    inline = re.search(r"^    needs: \[(.+)\]\s*$", block, re.MULTILINE)
    if inline:
        return [name.strip() for name in inline.group(1).split(",") if name.strip()]
    listed = re.search(r"^    needs:\n((?:      - .+\n?)+)", block, re.MULTILINE)
    if listed:
        return [line.strip().removeprefix("- ").strip() for line in listed.group(1).splitlines()]
    return []


def trigger_types(text: str) -> list[str]:
    match = re.search(r"^    types: \[(.+)\]\s*$", trigger_block(text), re.MULTILINE)
    if match is None:
        return []
    return [name.strip() for name in match.group(1).split(",") if name.strip()]


def planner_gates(text: str) -> list[str]:
    """The GATES tuple scripts/ci/quality-gate-plan.py declares."""
    match = re.search(r"^GATES = \(\n(.*?)^\)", text, re.MULTILINE | re.DOTALL)
    if match is None:
        return []
    return re.findall(r'"([a-z_]+)"', match.group(1))


def tree_proof_jobs(text: str) -> list[str]:
    """The REQUIRED_JOBS list scripts/ci/tree-already-proved.sh demands."""
    match = re.search(r"^REQUIRED_JOBS=\(\n(.*?)^\)", text, re.MULTILINE | re.DOTALL)
    if match is None:
        return []
    return [line.strip() for line in match.group(1).splitlines() if line.strip()]


def routed_by(block: str) -> list[str]:
    """The planner outputs a job's `if` reads."""
    return re.findall(r"needs\.impact\.outputs\.([a-z_]+) == 'true'", block)


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
    jobs = workflow_jobs(quality)
    gates = planner_gates(sources[PLANNER])
    if not gates:
        failures.append(f"{PLANNER} no longer declares a GATES tuple to read")
    missing_jobs = [job for job in UNROUTED_JOBS if job not in jobs]
    if missing_jobs:
        failures.append("quality-gate lost: " + ", ".join(missing_jobs))

    impact = job_block(quality, "impact")
    if impact is None:
        failures.append("quality-gate lost its impact stand-down job")
    else:
        if "github.event.pull_request.draft == false" not in impact:
            failures.append("quality-gate impact job lost its draft stand-down guard")
        if "needs.tree-proof.outputs.skip != 'true'" not in impact:
            failures.append("quality-gate impact job no longer honours the tree proof")
        # Both self-tests run before anything is planned: the router proves
        # its routing table and this contract proves itself, in the job whose
        # outputs every other job obeys.
        if "python3 scripts/ci/quality-gate-plan.py --self-test" not in impact:
            failures.append("quality-gate no longer proves its routing matrix first")
        if "python3 scripts/ci/dev-loop-workflow-contract.py --self-test" not in impact:
            failures.append("quality-gate no longer proves this contract first")
        if "--github-output" not in impact:
            failures.append("quality-gate impact job no longer publishes a plan")
        # Every planning invocation records what it planned, or the tree
        # proof's audit trail has a hole the run cannot be read back through.
        if impact.count("--step-summary") != impact.count("--github-output"):
            failures.append(
                "quality-gate impact job publishes a plan it does not record: "
                f"{impact.count('--github-output')} --github-output vs "
                f"{impact.count('--step-summary')} --step-summary"
            )
        for gate in gates:
            if f"      {gate}: ${{{{ steps.plan.outputs.{gate} }}}}" not in impact:
                failures.append(f"quality-gate impact job stopped exporting {gate}")
        if "      full: ${{ steps.plan.outputs.full }}" not in impact:
            failures.append("quality-gate impact job stopped exporting full")

    proof = job_block(quality, "tree-proof")
    if proof is None:
        failures.append("quality-gate lost its tree-proof job")
    else:
        if "if: github.event_name == 'push'" not in proof:
            failures.append("tree-proof must only answer for pushes")
        if "run: bash scripts/ci/tree-already-proved.sh quality-gate.yml" not in proof:
            failures.append("tree-proof no longer runs the tree proof script")
        if "run: bash scripts/ci/tree-already-proved.sh --self-test" not in proof:
            failures.append(
                "tree-proof no longer proves the rules by which it accepts a "
                "proof; they are what decides whether the gate runs at all"
            )

    # The proof accepts a run only when every job of the full matrix went
    # green, so its idea of "the full matrix" is the workflow's job list —
    # minus tree-proof, which is green on a run that skipped everything.
    expected_proof_jobs = sorted(job for job in jobs if job != "tree-proof")
    actual_proof_jobs = sorted(tree_proof_jobs(sources[TREE_PROOF]))
    if jobs and actual_proof_jobs != expected_proof_jobs:
        failures.append(
            f"{TREE_PROOF} demands a different full matrix than the workflow "
            f"runs: {actual_proof_jobs} vs {expected_proof_jobs}"
        )

    # --- every other job is a routed gate, and the planner knows it -------
    routed: dict[str, str] = {}
    for job in jobs:
        if job in UNROUTED_JOBS:
            continue
        block = job_block(quality, job)
        if block is None:
            failures.append(f"quality-gate job {job} could not be read")
            continue
        if "impact" not in needs_list(block):
            failures.append(
                f"quality-gate job {job} does not need impact, so it would "
                "run on a draft"
            )
        outputs = routed_by(block)
        if not outputs:
            failures.append(
                f"quality-gate job {job} is not routed by any planner output, "
                "so it runs whatever the change was"
            )
        else:
            routed[job] = outputs[0]
            unknown = [output for output in outputs if output not in gates]
            if unknown:
                failures.append(
                    f"quality-gate job {job} reads planner outputs that do not "
                    "exist: " + ", ".join(unknown)
                )
        if "!cancelled()" in block and "needs.impact.result == 'success'" not in block:
            failures.append(
                f"quality-gate job {job} opens with !cancelled() but does not "
                "require the planner to have succeeded, so it would run on a draft"
            )
        command = WORKSPACE_COMMANDS.get(job)
        if command is not None and command not in block:
            failures.append(
                f"quality-gate job {job} no longer runs `{command}`: a gate "
                "that compiles a subset is a proof about a subset"
            )

    unwired = sorted(set(gates) - set(routed.values()))
    if gates and unwired:
        failures.append(
            "the planner routes gates no job reads: " + ", ".join(unwired)
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
        if '"${result}" == "cancelled"' not in gate:
            failures.append(
                "gate no longer treats a cancelled job as a failure, so a "
                "cancelled matrix reports green"
            )
        expected_needs = sorted(job for job in jobs if job != "gate")
        actual_needs = sorted(needs_list(gate))
        if jobs and actual_needs != expected_needs:
            failures.append(
                "gate waits for a different set of jobs than the workflow "
                f"runs: {actual_needs} vs {expected_needs}"
            )

    # --- the rest of the full gate stands down too ------------------------
    for workflow in (QUALITY_GATE, *STANDDOWN_WORKFLOWS):
        missing_types = [
            name
            for name in REQUIRED_TRIGGER_TYPES
            if name not in trigger_types(sources[workflow])
        ]
        if missing_types:
            failures.append(
                f"{workflow} no longer triggers on " + ", ".join(missing_types)
                + ": the draft/ready handover would wait for the next push"
            )

    for workflow, standdown_jobs in STANDDOWN_WORKFLOWS.items():
        text = sources[workflow]
        for job in standdown_jobs:
            block = job_block(text, job)
            if block is None:
                failures.append(f"{workflow} lost the {job} job")
            elif READY_ONLY not in block:
                failures.append(f"{workflow} job {job} lost its draft stand-down guard")

    # --- every workflow is read, and every action is pinned ---------------
    on_disk = sorted(
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / ".github" / "workflows").glob("*.yml")
    )
    unread = [path for path in on_disk if path not in SOURCES]
    if unread:
        failures.append(
            "workflows this contract never reads: " + ", ".join(unread)
        )
    for workflow in (path for path in SOURCES if path.startswith(".github/")):
        for reference in ACTION_REFERENCE.findall(sources[workflow]):
            if reference.startswith("./"):
                continue
            _, _, pin = reference.partition("@")
            if not COMMIT_SHA.fullmatch(pin):
                failures.append(
                    f"{workflow} uses {reference}, which is a tag, not a "
                    "commit: whoever can move the tag decides what runs"
                )

    # --- the packaging matrix reads, the release upload writes ------------
    packaging = job_block(sources[PLUGIN_PACKAGE], PACKAGING_JOB)
    if packaging is None:
        failures.append(f"{PLUGIN_PACKAGE} lost the {PACKAGING_JOB} job")
    elif "contents: write" in packaging:
        failures.append(
            f"{PLUGIN_PACKAGE} job {PACKAGING_JOB} holds a write token while "
            "running scripts the pull request wrote"
        )
    elif "contents: read" not in packaging:
        failures.append(
            f"{PLUGIN_PACKAGE} job {PACKAGING_JOB} no longer names its "
            "permissions, so it inherits whatever the repository grants"
        )

    if '- "!crates/made-mcp/**/*.md"' not in trigger_block(sources[PLUGIN_PACKAGE]):
        failures.append(
            f"{PLUGIN_PACKAGE} watches crates/made-mcp/** without excluding "
            "its prose, so a README wakes four packaging hosts"
        )

    release = job_block(sources[PLUGIN_PACKAGE], RELEASE_JOB)
    if release is None:
        failures.append(f"{PLUGIN_PACKAGE} lost the {RELEASE_JOB} job")
    else:
        if "startsWith(github.ref, 'refs/tags/v')" not in release:
            failures.append(
                f"{PLUGIN_PACKAGE} job {RELEASE_JOB} is no longer tag-gated, "
                "so a write token reaches every push"
            )
        if "contents: write" not in release:
            failures.append(
                f"{PLUGIN_PACKAGE} job {RELEASE_JOB} cannot write the release "
                "it exists to upload"
            )
        if PACKAGING_JOB not in needs_list(release):
            failures.append(
                f"{PLUGIN_PACKAGE} job {RELEASE_JOB} no longer waits for "
                f"{PACKAGING_JOB}, so it would publish a partial release"
            )

    # --- H5: `just dev` is the same script, `just check` runs this --------
    justfile = sources[JUSTFILE]
    if "bash scripts/ci/dev-loop.sh {{STAGE}}" not in justfile:
        failures.append(
            "`just dev` no longer runs scripts/ci/dev-loop.sh, so the local "
            "loop and the workflow are two lists again"
        )
    if "python3 scripts/ci/quality-gate-plan.py --self-test" not in justfile:
        failures.append("`just workflow-contract` no longer proves the routing matrix")
    if "bash scripts/ci/tree-already-proved.sh --self-test" not in justfile:
        failures.append("`just workflow-contract` no longer proves the tree proof")
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
    "the tree proof stops proving its own rules": (
        QUALITY_GATE,
        "      - name: Prove the proof's own rules\n"
        "        run: bash scripts/ci/tree-already-proved.sh --self-test\n",
        "",
    ),
    "an action goes back to a movable tag": (
        PLUGIN_PACKAGE,
        "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
        "actions/upload-artifact@v4",
    ),
    "the packaging matrix gets a write token again": (
        PLUGIN_PACKAGE,
        "    permissions:\n      contents: read\n",
        "    permissions:\n      contents: write\n",
    ),
    "the release upload stops being tag-gated": (
        PLUGIN_PACKAGE,
        "  release-upload:\n    needs: [package]\n"
        "    if: startsWith(github.ref, 'refs/tags/v')\n",
        "  release-upload:\n    needs: [package]\n",
    ),
    "a README under made-mcp wakes four packaging hosts": (
        PLUGIN_PACKAGE,
        '      - "!crates/made-mcp/**/*.md"\n',
        "",
    ),
    "the container suites burn runners on drafts": (
        INTEGRATION,
        f"    {READY_ONLY}\n",
        "",
    ),
    # --- the four drifts the read-only review of #57/#58 found -----------
    # A job added to the workflow and to nobody's list: no impact guard, no
    # planner output, absent from the required context's needs.
    "a gate job answers to nobody": (
        QUALITY_GATE,
        "  architecture:\n    needs: [impact]\n",
        "  smuggled-gate:\n"
        "    runs-on: ubuntu-24.04\n"
        "    timeout-minutes: 5\n"
        "    steps:\n"
        "      - name: Do as it pleases\n"
        "        run: echo unguarded\n"
        "\n"
        "  architecture:\n    needs: [impact]\n",
    ),
    "a gate job reads a planner output that does not exist": (
        QUALITY_GATE,
        "needs.impact.outputs.architecture == 'true'",
        "needs.impact.outputs.arch == 'true'",
    ),
    "the planner grows a gate no job runs": (
        PLANNER,
        '    "publish",\n)',
        '    "publish",\n    "smoke",\n)',
    ),
    "the handover shrinks to ready_for_review alone": (
        QUALITY_GATE,
        "    types: [opened, synchronize, reopened, ready_for_review]\n",
        "    types: [ready_for_review]\n",
    ),
    "the contract stops proving itself": (
        QUALITY_GATE,
        "          python3 scripts/ci/dev-loop-workflow-contract.py --self-test\n",
        "",
    ),
    "the required context forgives a cancelled job": (
        QUALITY_GATE,
        '|| "${result}" == "cancelled"',
        "",
    ),
    # --- and the rules the four drifts made it worth deriving ------------
    "the planner stops recording what it planned": (
        QUALITY_GATE,
        '              --step-summary "${GITHUB_STEP_SUMMARY}"\n',
        "",
    ),
    "clippy stops proving the whole workspace": (
        QUALITY_GATE,
        "cargo clippy --workspace --all-targets --locked",
        "cargo clippy -p made-core --all-targets --locked",
    ),
    "test stops proving the whole workspace": (
        QUALITY_GATE,
        "cargo test --workspace --locked",
        "cargo test -p made-core --locked",
    ),
    "the tree proof forgets a job of the full matrix": (
        TREE_PROOF,
        "  coverage\n",
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

    routed = len(
        [job for job in workflow_jobs(sources[QUALITY_GATE]) if job not in UNROUTED_JOBS]
    )
    print(
        "dev-loop workflow contract passed: "
        f"{len(DEV_LOOP_JOBS)} draft-only lanes, "
        f"{routed} routed gate jobs derived from the workflow, "
        "one required gate context, DEV_PACKAGES agreed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
