#!/usr/bin/env python3
"""Plan the smallest fail-closed quality gate for a change (plan §3.9, H3).

A pull request pays for the parts of the product it can affect and nothing
else. Two rules decide that:

* Rust gates follow the **reverse workspace dependency closure**. A change
  inside a crate affects that crate and everything that depends on it,
  transitively, read from the manifests rather than guessed. The closure
  decides *which gates run*, never which crates they build: every Rust job
  is `--workspace`, because a gate that compiles a subset is a proof about
  a subset. `affected_packages` is the reason the plan gives for its
  booleans, not a cargo argument.
* The independent contracts — proto/AsyncAPI, the embedded boundaries, the
  plugin bundle, the chart, the container image, coverage, the publication
  dry run — follow **path routing**.

Both rules fail closed. A path this router does not recognise, a change to
the workspace manifest, lockfile or toolchain, a change to
`quality-gate.yml`, and a change to this file all return the full matrix; so
does `workflow_dispatch`. Being wrong must cost time, never safety.

Run it:
    python3 scripts/ci/quality-gate-plan.py --self-test
    python3 scripts/ci/quality-gate-plan.py --base <sha> --head <sha>
    python3 scripts/ci/quality-gate-plan.py --full --github-output "$GITHUB_OUTPUT"
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import subprocess
import sys
import tomllib
from collections import defaultdict, deque

ROOT = pathlib.Path(__file__).resolve().parents[2]

# One key per job in .github/workflows/quality-gate.yml, named after the job
# it routes. The workflow reads them as `needs.impact.outputs.<gate>`.
GATES = (
    "architecture",
    "contract",
    "rustfmt",
    "embedded_boundary",
    "embedded_sqlite",
    "clippy",
    "test",
    "coverage",
    "container",
    "helm",
    "benches",
    "publish",
)

# Gates that any source change inside any crate must run: they are
# workspace-wide by construction (`cargo fmt --all`, the architecture
# ratchet over every production file, `cargo bench --workspace --no-run`,
# `cargo llvm-cov --workspace`).
WORKSPACE_WIDE = ("architecture", "rustfmt", "clippy", "test", "coverage", "benches")

# Crates whose presence in the affected closure switches on a contract that
# is otherwise independent of the Rust matrix.
EMBEDDED_BOUNDARY_CRATES = {"made-embedded", "made-mcp"}
EMBEDDED_SQLITE_CRATES = {"made-adapters", "made-embedded", "made-mcp"}
CONTAINER_CRATES = {"made"}
PUBLISHED_CRATES = {"made-mcp", "made-mcp-proto"}

# Files that are documentation or fixtures to a reader and source code to
# rustc, because some crate bakes them in with `include_str!` /
# `include_bytes!`. Editing one changes what the workspace compiles and
# what its tests assert, so it pays for the two jobs that compile the
# crates' test targets and run them: `test` runs the assertion,
# `clippy --all-targets` compiles it. Not `coverage`: it re-runs the very
# tests `test` has already proved, and the line-coverage floor is a
# property of Rust sources, which these files are not.
EMBEDDED_DATA_GATES = ("clippy", "test")

FRAGMENT_SYNC_PATHS = {
    "api/examples/ceremonies/fragments/roundtable_fixed_order.yaml",
    "crates/made-app/src/usecases/fragments/roundtable_fixed_order.yaml",
    "crates/made-mcp/src/protocol/fragments/roundtable_fixed_order.yaml",
}

# Changing any of these changes what "proved" means, so the answer is the
# whole matrix rather than a cleverer plan.
FULL_MATRIX_PATHS = {
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".github/workflows/quality-gate.yml",
    "scripts/ci/quality-gate-plan.py",
    "scripts/ci/tree-already-proved.sh",
    "scripts/ci/install-protoc.sh",
}

# path prefix or exact path -> the gates it switches on. An empty tuple
# means "recognised, gates nothing" — documentation, the developer loop,
# the workflows that gate themselves, the manual E2E surface.
PREFIX_ROUTES: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("charts/", ("helm",)),
    ("specs/asyncapi/", ("contract",)),
    # Canonical fragment edits compile both consumers, prove the MCP build
    # without the embedded engine, and compare the packaged copies byte for
    # byte. Keep this before the broader api/ route.
    (
        "api/examples/ceremonies/fragments/",
        EMBEDDED_DATA_GATES + ("embedded_boundary", "contract"),
    ),
    ("api/", ("contract",)),
    ("plugins/", ("embedded_sqlite",)),
    ("tests/plugin/", ("embedded_sqlite",)),
    ("scripts/plugin/", ("embedded_sqlite",)),
    ("scripts/ci/made-plugin-", ("embedded_sqlite",)),
    ("scripts/ci/made-marketplace-contract.py", ("embedded_sqlite",)),
    (".claude-plugin/", ("embedded_sqlite",)),
    (".agents/plugins/", ("embedded_sqlite",)),
    ("scripts/release/", ("publish",)),
    ("scripts/ci/e2e-", ()),
    ("scripts/ci/integration-", ()),
    ("scripts/mcp/", ()),
    # The ceremony definitions are not test data on the side. They are
    # `include_str!`'d into made-e2e-runner's own sources, into seven
    # made-tests-integration tests and into two made-adapters unit tests,
    # and read from disk by two more made-adapters tests. Editing one
    # changes what the workspace compiles and what it asserts.
    ("tests/e2e/ceremonies/", EMBEDDED_DATA_GATES),
    ("tests/e2e/kubernetes/", ()),
    ("tests/cluster/", ()),
    (".kmp/", ()),
    (".github/", ()),
    ("docs/", ()),
)

EXACT_ROUTES: dict[str, tuple[str, ...]] = {
    "Dockerfile": ("container",),
    ".dockerignore": ("container",),
    "buf.yaml": ("contract",),
    "scripts/ci/container-image.sh": ("container",),
    "scripts/ci/build-provider-image.sh": ("container",),
    "scripts/ci/contract-gate.sh": ("contract",),
    "scripts/ci/install-buf.sh": ("contract",),
    "scripts/ci/install-asyncapi.sh": ("contract",),
    "scripts/ci/helm-lint.sh": ("helm",),
    "scripts/ci/install-helm.sh": ("helm",),
    "scripts/ci/architecture-gate.sh": ("architecture",),
    "docs/architecture/conformance.tsv": ("architecture",),
    # Documents the workspace compiles. `parity.tsv` is `include_str!`'d
    # by crates/made-mcp/src/protocol/parity_tests.rs and by
    # crates/made-tests-integration/tests/mcp_parity_session.rs;
    # `struct-numbers.tsv` by made-mcp and made-adapters tests; and
    # `support-matrix.md` by
    # crates/made-mcp/src/protocol/editions_matrix_tests.rs. All are
    # documentation to a reader and source to rustc, and
    # `check_embedded_data_routing` below fails if any stops being routed.
    "docs/architecture/parity.tsv": EMBEDDED_DATA_GATES,
    "docs/architecture/struct-numbers.tsv": EMBEDDED_DATA_GATES,
    "docs/operations/support-matrix.md": EMBEDDED_DATA_GATES,
    # The rest of the manual E2E surface, named one by one rather than by a
    # `tests/e2e/` prefix: nothing in the workspace compiles or reads these
    # and no CI job builds them, but a new directory or Dockerfile there
    # must fail closed to the full matrix rather than inherit an empty
    # route from its parent.
    "tests/e2e/docker-compose.e2e.yaml": (),
    "tests/e2e/provider-runner.Dockerfile": (),
    "tests/e2e/runner.Dockerfile": (),
    "tests/e2e/stub-llm.Dockerfile": (),
    "tests/e2e/stub-runtime.Dockerfile": (),
    "scripts/ci/domain-vocabulary-boundary.sh": ("rustfmt",),
    "scripts/ci/embedded-dependency-boundary.sh": ("embedded_boundary",),
    "scripts/ci/embedded-sqlite-gates.sh": ("embedded_sqlite",),
    "scripts/ci/bench-compile.sh": ("benches",),
    # Unlike KMP's, MADE's coverage job is self-contained — it runs
    # `cargo llvm-cov --workspace` itself rather than reducing fragments
    # other jobs upload. Changing the script therefore changes exactly one
    # gate, and routing to it is precise rather than optimistic.
    "scripts/ci/rust-coverage.sh": ("coverage",),
    "scripts/ci/publish-dry-run.sh": ("publish",),
    "scripts/ci/publish-crates.sh": ("publish",),
    "scripts/release.sh": ("publish",),
    "scripts/ci/testcontainers-host.sh": (),
    "scripts/ci/deploy-kubernetes.sh": (),
    "scripts/ci/quality-gate.sh": (),
    "scripts/ci/dev-loop.sh": (),
    "scripts/ci/dev-loop-workflow-contract.py": (),
    "justfile": (),
    "Makefile": (),
    ".gitignore": (),
    ".gitattributes": (),
    "LICENSE": (),
    "NOTICE": (),
}

DOC_SUFFIXES = (".md", ".txt")


def is_documentation(path: str) -> bool:
    return path.startswith("docs/") or path.endswith(DOC_SUFFIXES)


def workspace_graph() -> tuple[dict[str, pathlib.Path], dict[str, set[str]]]:
    """Package name -> directory, and package name -> its local dependencies."""
    manifests = sorted((ROOT / "crates").glob("*/Cargo.toml"))
    packages: dict[str, pathlib.Path] = {}
    documents: dict[str, dict[str, object]] = {}
    for manifest in manifests:
        body = tomllib.loads(manifest.read_text(encoding="utf-8"))
        name = str(body["package"]["name"])
        packages[name] = manifest.parent.relative_to(ROOT)
        documents[name] = body

    root = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    workspace_dependencies = root.get("workspace", {}).get("dependencies", {})
    local_names = set(packages)
    dependencies: dict[str, set[str]] = {name: set() for name in packages}

    def collect(table: object) -> set[str]:
        found: set[str] = set()
        if not isinstance(table, dict):
            return found
        for key, spec in table.items():
            candidate = key
            if isinstance(spec, dict):
                candidate = str(spec.get("package", key))
                if spec.get("workspace") is True:
                    inherited = workspace_dependencies.get(key, {})
                    if isinstance(inherited, dict):
                        candidate = str(inherited.get("package", key))
            if candidate in local_names:
                found.add(candidate)
        return found

    sections = ("dependencies", "dev-dependencies", "build-dependencies")
    for package, body in documents.items():
        for section in sections:
            dependencies[package].update(collect(body.get(section)))
        targets = body.get("target", {})
        if isinstance(targets, dict):
            for target in targets.values():
                if not isinstance(target, dict):
                    continue
                for section in sections:
                    dependencies[package].update(collect(target.get(section)))

    return packages, dependencies


def reverse_closure(changed: set[str], dependencies: dict[str, set[str]]) -> set[str]:
    """Every crate that can observe a change in `changed`, transitively."""
    reverse: dict[str, set[str]] = defaultdict(set)
    for package, package_dependencies in dependencies.items():
        for dependency in package_dependencies:
            reverse[dependency].add(package)
    affected = set(changed)
    queue = deque(sorted(changed))
    while queue:
        dependency = queue.popleft()
        for dependent in sorted(reverse[dependency]):
            if dependent not in affected:
                affected.add(dependent)
                queue.append(dependent)
    return affected


def empty_plan() -> dict[str, object]:
    return {
        "full": False,
        "reason": "path-specific",
        "changed_packages": [],
        "affected_packages": [],
        **{gate: False for gate in GATES},
    }


def full_plan(packages: dict[str, pathlib.Path], reason: str) -> dict[str, object]:
    names = sorted(packages)
    return {
        "full": True,
        "reason": reason,
        "changed_packages": names,
        "affected_packages": names,
        **{gate: True for gate in GATES},
    }


def route_path(path: str) -> tuple[str, ...] | None:
    if path in EXACT_ROUTES:
        return EXACT_ROUTES[path]
    for prefix, gates in PREFIX_ROUTES:
        if path.startswith(prefix):
            return gates
    if is_documentation(path):
        return ()
    return None


def plan_for(paths: list[str], force_full: bool = False) -> dict[str, object]:
    packages, dependencies = workspace_graph()
    if force_full:
        return full_plan(packages, "explicit full run")
    if not paths:
        return full_plan(packages, "no changed paths could be established")

    normalized = sorted({path.removeprefix("./") for path in paths if path})
    if any(path in FULL_MATRIX_PATHS for path in normalized):
        return full_plan(packages, "workspace, toolchain or routing contract changed")

    plan = empty_plan()
    known: set[str] = set()
    changed_packages: set[str] = set()

    # --- crates: the reverse dependency closure ---------------------------
    for name, directory in packages.items():
        prefix = directory.as_posix() + "/"
        crate_paths = {
            path
            for path in normalized
            if path.startswith(prefix) and not is_documentation(path)
        }
        if not crate_paths:
            continue
        changed_packages.add(name)
        known.update(crate_paths)
        # The proto contract lives inside two crates but is not Rust: the
        # `contract` job lints it, checks it for breaking changes, and
        # proves the vendored copy has not drifted.
        if any("/proto/" in path for path in crate_paths):
            plan["contract"] = True
        if FRAGMENT_SYNC_PATHS & crate_paths:
            plan["contract"] = True

    if changed_packages:
        affected = reverse_closure(changed_packages, dependencies)
        plan["changed_packages"] = sorted(changed_packages)
        plan["affected_packages"] = sorted(affected)
        for gate in WORKSPACE_WIDE:
            plan[gate] = True
        plan["embedded_boundary"] = bool(EMBEDDED_BOUNDARY_CRATES & affected)
        plan["embedded_sqlite"] = bool(EMBEDDED_SQLITE_CRATES & affected)
        plan["container"] = bool(CONTAINER_CRATES & affected)
        plan["publish"] = bool(PUBLISHED_CRATES & affected)

    # --- everything else: path routing ------------------------------------
    for path in normalized:
        if path in known:
            continue
        gates = route_path(path)
        if gates is None:
            continue
        known.add(path)
        for gate in gates:
            plan[gate] = True

    unknown = sorted(set(normalized) - known)
    if unknown:
        return full_plan(packages, "unknown paths: " + ", ".join(unknown))

    if changed_packages:
        plan["reason"] = "changed crates and their reverse dependencies"
    return plan


# --- the non-regressable rule -------------------------------------------
#
# A file a crate bakes in with `include_str!` / `include_bytes!` is source
# code, wherever it lives. While it lives inside the crate that includes
# it the crate prefix routes it; once it escapes, the tables above are the
# only thing between an edit and a gate that runs nothing. That is exactly
# how `docs/architecture/parity.tsv`, `docs/operations/support-matrix.md`
# and `tests/e2e/ceremonies/*.yaml` reached `main` unrouted.
#
# So the self-test walks every include in `crates/**`, resolves its target,
# and fails when an escaping target would run no Rust job. The table can
# fall behind the code once; it cannot fall behind it twice.

INCLUDE_LITERAL = re.compile(
    r'include_(?:str|bytes)!\s*\(\s*"((?:[^"\\]|\\.)*)"', re.S
)
INCLUDE_MANIFEST_DIR = re.compile(
    r'include_(?:str|bytes)!\s*\(\s*concat!\s*\(\s*env!\s*\(\s*'
    r'"CARGO_MANIFEST_DIR"\s*\)\s*,\s*"((?:[^"\\]|\\.)*)"',
    re.S,
)


def embedded_data_targets() -> dict[str, set[str]]:
    """Escaping include target -> the sources that compile it in.

    A target inside the including crate's own directory is left out: the
    crate prefix already routes it through the dependency closure.
    """
    escaping: dict[str, set[str]] = defaultdict(set)
    for manifest in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        crate = manifest.parent
        for source in sorted(crate.rglob("*.rs")):
            if "target" in source.relative_to(crate).parts:
                continue
            body = source.read_text(encoding="utf-8")
            targets = [
                (source.parent / literal) for literal in INCLUDE_LITERAL.findall(body)
            ]
            targets += [
                (crate / literal.lstrip("/"))
                for literal in INCLUDE_MANIFEST_DIR.findall(body)
            ]
            for target in targets:
                resolved = pathlib.Path(os.path.normpath(target))
                if resolved.is_relative_to(crate):
                    continue
                if not resolved.is_relative_to(ROOT):
                    raise SystemExit(
                        f"{source.relative_to(ROOT)} includes {target}, which "
                        "leaves the repository; nothing can route it"
                    )
                key = resolved.relative_to(ROOT).as_posix()
                escaping[key].add(source.relative_to(ROOT).as_posix())
    return escaping


def check_embedded_data_routing() -> int:
    """Every file a crate compiles in from elsewhere must reach `test`."""
    escaping = embedded_data_targets()
    unrouted = [
        f"{target} (compiled into {', '.join(sorted(sources))}) routes to no "
        "Rust job"
        for target, sources in sorted(escaping.items())
        if not plan_for([target])["test"]
    ]
    if unrouted:
        raise SystemExit(
            "quality gate plan self-test: a crate compiles in a file the "
            "router does not route:\n  - " + "\n  - ".join(unrouted)
        )
    return len(escaping)


# `--diff-filter=ACMR` used to ask this question, and it dropped the two
# statuses that matter most. A deletion is a change to the workspace —
# removing a crate source changes what compiles, and removing a routed file
# changes what a gate proves — and a rename's *source* path is where the
# routed file used to live. On 40cb7e5 the filter turned a sixteen-file diff
# into three, so a pull request that deletes a crate source and edits a .md
# routed to nothing at all.
#
# `-M --name-status` keeps every status and names both sides of a rename or
# a copy. The plan is the union of the two sides, which is the only answer
# that is right whichever side carried the gate.
def parse_name_status(output: str) -> list[str]:
    """Every path a `git diff -M --name-status` answer names, both sides."""
    paths: list[str] = []
    for line in output.splitlines():
        fields = [field for field in line.split("\t") if field]
        # `M\tpath`, `D\tpath`, `R100\told\tnew`, `C075\tsource\tcopy`.
        paths.extend(fields[1:])
    return paths


def changed_paths(base: str, head: str) -> list[str]:
    result = subprocess.run(
        ["git", "diff", "-M", "--name-status", base, head, "--"],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return parse_name_status(result.stdout)


def write_outputs(plan: dict[str, object], destination: pathlib.Path) -> None:
    with destination.open("a", encoding="utf-8") as handle:
        for key, value in plan.items():
            if isinstance(value, bool):
                rendered = str(value).lower()
            elif isinstance(value, list):
                rendered = json.dumps(value, separators=(",", ":"))
            else:
                rendered = str(value)
            print(f"{key}={rendered}", file=handle)


def write_step_summary(
    plan: dict[str, object], paths: list[str], event: str, destination: pathlib.Path
) -> None:
    """Record the plan in the run, where the tree proof's reader can find it.

    `scripts/ci/tree-already-proved.sh` decides whether an earlier run proved
    a tree from the run's *job conclusions*, which the run's own scripts
    cannot forge. This is the same statement in the form a person reads: what
    was planned, on which event, and why.
    """
    gates = ",".join(gate for gate in GATES if plan[gate]) or "none"
    with destination.open("a", encoding="utf-8") as handle:
        print("### quality gate plan", file=handle)
        print("", file=handle)
        print("```", file=handle)
        print(
            f"full={str(plan['full']).lower()} event={event} gates={gates}",
            file=handle,
        )
        print(f"reason={plan['reason']}", file=handle)
        print(f"changed paths={len(paths)}", file=handle)
        print("```", file=handle)


SELF_TEST_CASES: tuple[tuple[str, list[str], dict[str, object]], ...] = (
    # Acceptance, plan §3.9 H3: a docs-only change runs no Rust job.
    (
        "docs only",
        ["docs/orchestration-patterns-plan.md", "README.md", "crates/made-mcp/README.md"],
        {gate: False for gate in GATES} | {"full": False},
    ),
    # Acceptance, plan §3.9 H3: a made-core change runs everything that a
    # Rust change can reach — the whole matrix plus the embedded gates, the
    # container image and the publication dry run, because made-core is in
    # every crate's dependency tree. `contract` (buf lint, AsyncAPI) and
    # `helm` (helm lint over charts/) are the two gates no Rust source can
    # affect; routing them here would be a green light, not a proof.
    (
        "made-core",
        ["crates/made-core/src/lib.rs"],
        {
            gate: gate not in {"contract", "helm"}
            for gate in GATES
        }
        | {"full": False},
    ),
    (
        "changelog",
        ["CHANGELOG.md"],
        {"clippy": False, "coverage": False, "full": False},
    ),
    # Documentation that rustc compiles. These files are `include_str!`'d
    # into tests, so "docs-only" stops being the same thing as "no Rust
    # job" — and `check_embedded_data_routing` keeps that true.
    (
        "the parity file two crates compile in",
        ["docs/architecture/parity.tsv"],
        {"test": True, "clippy": True, "coverage": False, "helm": False, "full": False},
    ),
    (
        "the struct-number table two crates compile in",
        ["docs/architecture/struct-numbers.tsv"],
        {"test": True, "clippy": True, "coverage": False, "helm": False, "full": False},
    ),
    (
        "the support matrix made-mcp compiles in",
        ["docs/operations/support-matrix.md"],
        {"test": True, "clippy": True, "coverage": False, "full": False},
    ),
    (
        "a ceremony definition three crates compile in",
        ["tests/e2e/ceremonies/daily-standup.yaml"],
        {
            "test": True,
            "clippy": True,
            "architecture": False,
            "coverage": False,
            "container": False,
            "full": False,
        },
    ),
    (
        "a ceremony fragment made-app compiles in",
        ["api/examples/ceremonies/fragments/roundtable_fixed_order.yaml"],
        {
            "test": True,
            "clippy": True,
            "embedded_boundary": True,
            "contract": True,
            "coverage": False,
            "full": False,
        },
    ),
    (
        "the packaged made-app fragment stays synchronized",
        ["crates/made-app/src/usecases/fragments/roundtable_fixed_order.yaml"],
        {
            "test": True,
            "clippy": True,
            "embedded_boundary": True,
            "contract": True,
            "full": False,
        },
    ),
    (
        "the packaged grpc-only MCP fragment stays synchronized",
        ["crates/made-mcp/src/protocol/fragments/roundtable_fixed_order.yaml"],
        {
            "test": True,
            "clippy": True,
            "embedded_boundary": True,
            "contract": True,
            "full": False,
        },
    ),
    (
        "the manual E2E surface still gates nothing",
        [
            "tests/e2e/kubernetes/runner-job.yaml",
            "tests/e2e/docker-compose.e2e.yaml",
            "tests/e2e/runner.Dockerfile",
        ],
        {gate: False for gate in GATES} | {"full": False},
    ),
    (
        "a new directory under tests/e2e fails closed",
        ["tests/e2e/contracts/surface.yaml"],
        {"full": True},
    ),
    (
        "chart",
        ["charts/made/values.yaml"],
        {"helm": True, "clippy": False, "container": False, "full": False},
    ),
    (
        "container image",
        ["Dockerfile"],
        {"container": True, "clippy": False, "coverage": False, "full": False},
    ),
    (
        "proto contract",
        ["crates/made-proto/proto/underpass/made/v1/made.proto"],
        {"contract": True, "clippy": True, "full": False},
    ),
    (
        "asyncapi only",
        ["specs/asyncapi/made.asyncapi.yaml"],
        {"contract": True, "clippy": False, "coverage": False, "full": False},
    ),
    (
        "plugin bundle",
        ["plugins/made/skills/made-setup/SKILL.md"],
        {"embedded_sqlite": True, "clippy": False, "full": False},
    ),
    (
        "plugin smoke script",
        ["scripts/ci/made-plugin-smoke.sh"],
        {"embedded_sqlite": True, "clippy": False, "full": False},
    ),
    (
        "vendored mcp proto reaches publication",
        ["crates/made-mcp-proto/src/lib.rs"],
        {"publish": True, "container": False, "clippy": True, "full": False},
    ),
    (
        "server crate builds the image",
        ["crates/made/src/compose.rs"],
        {"container": True, "clippy": True, "full": False},
    ),
    (
        "adapters reach the embedded gates",
        ["crates/made-adapters/src/lib.rs"],
        {"embedded_sqlite": True, "embedded_boundary": True, "full": False},
    ),
    (
        "e2e runner does not reach the embedded gates",
        ["crates/made-e2e-runner/src/main.rs"],
        {"embedded_sqlite": False, "embedded_boundary": False, "clippy": True},
    ),
    (
        "coverage script",
        ["scripts/ci/rust-coverage.sh"],
        {"coverage": True, "clippy": False, "full": False},
    ),
    (
        "dev loop is not a quality gate",
        ["scripts/ci/dev-loop.sh", "justfile", ".github/workflows/dev-loop.yml"],
        {gate: False for gate in GATES} | {"full": False},
    ),
    ("workspace lock", ["Cargo.lock"], {"full": True, "coverage": True}),
    (
        "the router itself",
        ["scripts/ci/quality-gate-plan.py"],
        {"full": True, "coverage": True},
    ),
    (
        "the workflow itself",
        [".github/workflows/quality-gate.yml"],
        {"full": True, "coverage": True},
    ),
    (
        "the tree proof itself",
        ["scripts/ci/tree-already-proved.sh"],
        {"full": True},
    ),
    # A change boundary that drops deletions is how a pull request that
    # removes a crate source and edits a document routes to nothing.
    (
        "a deleted crate source still runs the Rust gates",
        ["crates/made-e2e-runner/src/scenarios/daily_standup.rs", "docs/dev-loop.md"],
        {"clippy": True, "test": True, "rustfmt": True, "full": False},
    ),
    ("unknown path", ["new-top-level.bin"], {"full": True}),
    ("no paths at all", [], {"full": True}),
)


# The change boundary itself, which is where the two statuses were lost.
NAME_STATUS_CASES: tuple[tuple[str, str, list[str]], ...] = (
    (
        "a deletion is a change",
        "D\tcrates/made-e2e-runner/src/scenarios/daily_standup.rs\n"
        "M\tdocs/dev-loop.md\n",
        ["crates/made-e2e-runner/src/scenarios/daily_standup.rs", "docs/dev-loop.md"],
    ),
    (
        "a rename routes the path it left as well as the one it reached",
        "R100\tcrates/made-core/src/old.rs\tcrates/made-app/src/new.rs\n",
        ["crates/made-core/src/old.rs", "crates/made-app/src/new.rs"],
    ),
    (
        "so does a copy",
        "C075\tdocs/architecture/parity.tsv\tdocs/architecture/spare.tsv\n",
        ["docs/architecture/parity.tsv", "docs/architecture/spare.tsv"],
    ),
    (
        "additions and modifications survive unchanged",
        "A\tcharts/made/values.yaml\nM\tCargo.lock\n",
        ["charts/made/values.yaml", "Cargo.lock"],
    ),
)


def self_test() -> None:
    for name, paths, expected in SELF_TEST_CASES:
        actual = plan_for(paths)
        for key, value in expected.items():
            if actual[key] != value:
                raise SystemExit(
                    f"quality gate plan self-test: {name}: expected "
                    f"{key}={value!r}, got {actual[key]!r}"
                )
    for name, output, expected in NAME_STATUS_CASES:
        actual = parse_name_status(output)
        if actual != expected:
            raise SystemExit(
                f"quality gate plan self-test: {name}: expected "
                f"{expected!r}, got {actual!r}"
            )
    embedded = check_embedded_data_routing()
    print(
        f"quality gate plan self-test passed: {len(SELF_TEST_CASES)} routing "
        f"cases, {len(NAME_STATUS_CASES)} change-boundary cases, {embedded} "
        "files compiled into a crate from outside it"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base")
    parser.add_argument("--head")
    parser.add_argument("--path", action="append", default=[])
    parser.add_argument("--full", action="store_true")
    parser.add_argument("--github-output", type=pathlib.Path)
    parser.add_argument("--step-summary", type=pathlib.Path)
    parser.add_argument("--event", default=os.environ.get("GITHUB_EVENT_NAME", "local"))
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0

    if args.path:
        paths = args.path
    elif args.base and args.head:
        paths = changed_paths(args.base, args.head)
    elif args.full:
        paths = []
    else:
        parser.error("provide --base/--head, --path, --full, or --self-test")

    plan = plan_for(paths, force_full=args.full)
    if args.github_output:
        write_outputs(plan, args.github_output)
    if args.step_summary:
        write_step_summary(plan, paths, args.event, args.step_summary)
    print(json.dumps({"paths": sorted(paths), "plan": plan}, indent=2), file=sys.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
