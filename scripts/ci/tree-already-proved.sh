#!/usr/bin/env bash
# Was this exact working tree already proved green? (plan §3.9, H4)
#
# A merge to main usually lands a tree byte-identical to the pull request
# head that was just proved: same tree hash, different commit sha. The gates
# compile and test a working tree, so running them again proves nothing —
# while doubling the exposure to the flaky parts of the matrix (Docker,
# testcontainers, registry reachability). Every false red on a tree that was
# green minutes earlier trains the reflex to re-run rather than read.
#
# The rule is "skip when this tree was already proved", never "trust the
# pull request". A merge from an out-of-date branch, a conflict resolved in
# the GitHub UI and a direct push each produce a tree nobody tested, and each
# gets the full gate. So does any doubt at all: an API that will not answer,
# a run whose commit cannot be resolved, no green run to compare against.
#
# "Proved" is the part this script has to get exactly right, because a proof
# it accepts wrongly makes the hole permanent. A successful run is not a
# proof. A run is a proof of its tree only when all four of these hold:
#
#   * it ran in **this** repository — `head_repository.full_name` equals
#     `GITHUB_REPOSITORY`, so a fork's run never proves anything here;
#   * it is younger than TREE_PROOF_MAX_AGE_DAYS — a tree proved against a
#     months-old toolchain, action pin or dependency graph is not proved
#     against today's;
#   * **every job of the full matrix concluded `success`** — not skipped,
#     not cancelled. This is the whole of the plan check: a partial plan
#     skips at least one gate, so its run cannot pass this test, and neither
#     can a push whose own tree proof skipped the gate. It is read from the
#     run's job conclusions rather than from the plan the run recorded,
#     because on `pull_request` the planner runs from the pull request's own
#     head and its word for "full" is the pull request's word;
#   * its event is `push` or `workflow_dispatch`, or — as above — its full
#     matrix ran green anyway. The `impact` job records `full`, the gates and
#     the event in the run summary, which is where a person reads the same
#     statement the job conclusions make to this script.
#
# Usage: bash scripts/ci/tree-already-proved.sh [workflow-file]
#        bash scripts/ci/tree-already-proved.sh --self-test
# Writes `skip=true|false` to $GITHUB_OUTPUT (stdout outside Actions).
#
# Deliberately not `set -e`: every failure path here answers "run the gates"
# rather than breaking the build.
set -uo pipefail

RUNS_TO_CHECK="${TREE_PROOF_RUNS:-30}"
MAX_AGE_DAYS="${TREE_PROOF_MAX_AGE_DAYS:-14}"

# Every job a full plan runs, and therefore every job a proof must show
# green. `tree-proof` is deliberately absent: it is green on a push that
# skipped the whole gate, so it discriminates nothing.
# scripts/ci/dev-loop-workflow-contract.py fails the build if this list ever
# stops matching .github/workflows/quality-gate.yml.
REQUIRED_JOBS=(
  impact
  architecture
  contract
  rustfmt
  embedded-boundary
  embedded-sqlite-gates
  clippy
  test
  coverage
  container-image
  helm-chart
  benches-compile
  publish-dry-run
  gate
)

say() { printf '%s\n' "$*" >&2; }
answer() { printf 'skip=%s\n' "$1" >>"${GITHUB_OUTPUT:-/dev/stdout}"; exit 0; }

# The verdict is arithmetic over four fields and a job list, so it is one
# python3 expression rather than four fragile shell comparisons — and the
# self-test below drives exactly this function with fixture answers, so what
# is tested is what runs.
PROOF_PY=$(
  cat <<'PY'
import json
import sys
from datetime import datetime, timezone

event, head_repository, created_at, repository, max_days, required, jobs = sys.argv[1:8]


def reject(reason: str) -> None:
    print(reason)
    raise SystemExit(1)


if head_repository != repository:
    reject(f"it ran in {head_repository or '(no head repository)'}, not {repository}")

try:
    created = datetime.fromisoformat(created_at.replace("Z", "+00:00"))
except ValueError:
    reject(f"its age cannot be read from {created_at!r}")
age = (datetime.now(timezone.utc) - created).total_seconds() / 86400.0
if age > float(max_days):
    reject(f"it is {age:.1f} days old, past the {max_days}-day cap")

try:
    conclusions = {job["name"]: job.get("conclusion") for job in json.loads(jobs)}
except (json.JSONDecodeError, TypeError, KeyError):
    reject("its job list could not be read")

unproved = [name for name in required.split() if conclusions.get(name) != "success"]
if unproved:
    reject(
        "these jobs of the full matrix did not succeed: "
        + ", ".join(f"{name}={conclusions.get(name) or 'absent'}" for name in unproved)
    )

if event in ("push", "workflow_dispatch"):
    print(f"a {event} run whose full matrix passed")
else:
    print(f"a {event} run whose full matrix passed anyway")
PY
)

# $1 event, $2 head repository, $3 created_at, $4 the run's jobs as JSON.
# Prints the reason either way; exit 0 means "this is a proof".
verdict() {
  python3 -c "${PROOF_PY}" \
    "$1" "$2" "$3" "${REPO}" "${MAX_AGE_DAYS}" "${REQUIRED_JOBS[*]}" "$4"
}

# --- self-test ---------------------------------------------------------
#
# Fixtures stand in for the three API answers, so the six cases below run
# offline and drive the same `verdict` the live path drives.

fixture_jobs() {
  python3 -c '
import json, sys
required = sys.argv[1].split()
overrides = dict(pair.split("=", 1) for pair in sys.argv[2:])
print(json.dumps([
    {"name": name, "conclusion": overrides.get(name, "success")}
    for name in required
]))
' "${REQUIRED_JOBS[*]}" "$@"
}

fixture_age_days() {
  python3 -c '
import sys
from datetime import datetime, timedelta, timezone
moment = datetime.now(timezone.utc) - timedelta(days=float(sys.argv[1]))
print(moment.isoformat().replace("+00:00", "Z"))
' "$1"
}

self_test() {
  REPO="underpass-ai/made"
  local failures=0 fresh stale full partial skipped cancelled

  fresh="$(fixture_age_days 0.02)"
  stale="$(fixture_age_days 30)"
  full="$(fixture_jobs)"
  # What a docs-only pull request produced before this change: the planner
  # and the required context are green, every gate is skipped.
  partial="$(fixture_jobs architecture=skipped contract=skipped rustfmt=skipped \
    embedded-boundary=skipped embedded-sqlite-gates=skipped clippy=skipped \
    test=skipped coverage=skipped container-image=skipped helm-chart=skipped \
    benches-compile=skipped publish-dry-run=skipped)"
  skipped="$(fixture_jobs coverage=skipped)"
  cancelled="$(fixture_jobs test=cancelled)"

  expect() {
    local name="$1" want="$2" reason
    shift 2
    if reason="$(verdict "$@")"; then
      [ "${want}" = "accept" ] && { say "  ok   ${name}: ${reason}"; return; }
      say "  FAIL ${name}: accepted, and should not have (${reason})"
    else
      [ "${want}" = "reject" ] && { say "  ok   ${name}: rejected — ${reason}"; return; }
      say "  FAIL ${name}: rejected, and should not have (${reason})"
    fi
    failures=$((failures + 1))
  }

  say "tree proof self-test (repository ${REPO}, ${MAX_AGE_DAYS}-day cap)"
  expect "a partial pull-request plan is not a proof" reject \
    pull_request "${REPO}" "${fresh}" "${partial}"
  expect "a full pull-request plan is a proof" accept \
    pull_request "${REPO}" "${fresh}" "${full}"
  expect "a push whose matrix ran is a proof" accept \
    push "${REPO}" "${fresh}" "${full}"
  expect "a fork's run proves nothing here" reject \
    pull_request "someone-else/made" "${fresh}" "${full}"
  expect "a proof older than the cap has expired" reject \
    push "${REPO}" "${stale}" "${full}"
  expect "a skipped job is not a passed job" reject \
    push "${REPO}" "${fresh}" "${skipped}"
  expect "neither is a cancelled one" reject \
    push "${REPO}" "${fresh}" "${cancelled}"

  if [ "${failures}" -ne 0 ]; then
    say "tree proof self-test: ${failures} case(s) failed"
    exit 1
  fi
  say "tree proof self-test: 7 cases hold"
  exit 0
}

if [ "${1:-}" = "--self-test" ]; then
  self_test
fi

WORKFLOW="${1:-quality-gate.yml}"

TREE="$(git rev-parse HEAD^{tree} 2>/dev/null)"
if [ -z "${TREE}" ]; then
  say "cannot resolve this tree; running the gates"
  answer false
fi
say "this tree: ${TREE}"

REPO="${GITHUB_REPOSITORY:-$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null)}"
if [ -z "${REPO}" ]; then
  say "cannot resolve the repository; running the gates"
  answer false
fi

# Successful runs of this same workflow, newest first. A run proves the tree
# of the commit it ran on, whatever branch that commit was on — which is the
# whole point: the proof came from the pull request head.
RUNS="$(gh api "repos/${REPO}/actions/workflows/${WORKFLOW}/runs?status=success&per_page=${RUNS_TO_CHECK}" \
  --jq '.workflow_runs[] | [.id, .head_sha, .event, (.head_repository.full_name // ""), .created_at] | @tsv' \
  2>/dev/null)"
if [ -z "${RUNS}" ]; then
  say "no successful runs to compare against; running the gates"
  answer false
fi

while IFS=$'\t' read -r id sha event head_repository created_at; do
  [ -z "${sha}" ] && continue
  [ "${sha}" = "${GITHUB_SHA:-}" ] && continue
  proved="$(gh api "repos/${REPO}/commits/${sha}" --jq '.commit.tree.sha' 2>/dev/null)"
  [ "${proved}" = "${TREE}" ] || continue

  jobs="$(gh api "repos/${REPO}/actions/runs/${id}/jobs?per_page=100" \
    --jq '[.jobs[] | {name: .name, conclusion: .conclusion}]' 2>/dev/null)"
  if reason="$(verdict "${event}" "${head_repository}" "${created_at}" "${jobs:-}")"; then
    say "tree already proved green by ${sha} (run ${id}): ${reason}"
    answer true
  fi
  say "run ${id} ran on this tree but is not a proof: ${reason}"
done <<EOF
${RUNS}
EOF

say "no green run proves this tree; running the gates"
answer false
