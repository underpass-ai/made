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
# Usage: bash scripts/ci/tree-already-proved.sh [workflow-file]
# Writes `skip=true|false` to $GITHUB_OUTPUT (stdout outside Actions).
#
# Deliberately not `set -e`: every failure path here answers "run the gates"
# rather than breaking the build.
set -uo pipefail

WORKFLOW="${1:-quality-gate.yml}"
RUNS_TO_CHECK="${TREE_PROOF_RUNS:-30}"

say() { printf '%s\n' "$*" >&2; }
answer() { printf 'skip=%s\n' "$1" >>"${GITHUB_OUTPUT:-/dev/stdout}"; exit 0; }

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
SHAS="$(gh api "repos/${REPO}/actions/workflows/${WORKFLOW}/runs?status=success&per_page=${RUNS_TO_CHECK}" \
  --jq '.workflow_runs[].head_sha' 2>/dev/null)"
if [ -z "${SHAS}" ]; then
  say "no successful runs to compare against; running the gates"
  answer false
fi

for sha in ${SHAS}; do
  [ "${sha}" = "${GITHUB_SHA:-}" ] && continue
  proved="$(gh api "repos/${REPO}/commits/${sha}" --jq '.commit.tree.sha' 2>/dev/null)"
  if [ "${proved}" = "${TREE}" ]; then
    say "tree already proved green by ${sha}"
    answer true
  fi
done

say "no green run covers this tree; running the gates"
answer false
