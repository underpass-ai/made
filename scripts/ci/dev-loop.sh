#!/usr/bin/env bash
# The developer loop: the same commands locally and in CI.
#
# While a pull request is a draft the full quality gate stands down and
# this loop is the answer — formatting, lint and tests for the crates
# under work, plus the three architecture gates that cost seconds. It is
# fast feedback, not proof: nothing merges on this loop's word. Marking
# the pull request ready for review wakes the full gate.
#
# `just dev` and `.github/workflows/dev-loop.yml` both call this script,
# so "the local loop and CI run the same thing" is literal, not a claim
# about two lists that drift.
#
# Usage:
#   bash scripts/ci/dev-loop.sh          # every stage, in order
#   bash scripts/ci/dev-loop.sh lint     # fmt + clippy
#   bash scripts/ci/dev-loop.sh test     # tests of DEV_PACKAGES
#   bash scripts/ci/dev-loop.sh gates    # the three seconds-long gates
#
# DEV_PACKAGES names the crates the current phase iterates on. Widen or
# narrow it per phase; the full gate on ready-for-review still proves the
# whole workspace. Override it for one run:
#
#   DEV_PACKAGES="-p made-mcp" bash scripts/ci/dev-loop.sh
#
# The default below is the single source of truth: the workflow sets the
# same value and scripts/ci/dev-loop-workflow-contract.py fails if the
# two ever disagree.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

DEV_PACKAGES="${DEV_PACKAGES:--p made-core -p made-app -p made-adapters -p made-embedded -p made-mcp}"

STAGE="${1:-all}"

say() { printf '\n== dev loop: %s ==\n' "$*"; }

# Word splitting on DEV_PACKAGES is the point: it is a list of cargo flags.
# shellcheck disable=SC2086

stage_lint() {
  say "formatting"
  cargo fmt --all -- --check

  say "clippy on ${DEV_PACKAGES}"
  cargo clippy ${DEV_PACKAGES} --all-targets --locked -- -D warnings
}

stage_test() {
  say "tests of ${DEV_PACKAGES}"
  cargo test ${DEV_PACKAGES} --locked
}

stage_gates() {
  say "architecture ratchet"
  bash scripts/ci/architecture-gate.sh

  say "domain vocabulary boundary"
  bash scripts/ci/domain-vocabulary-boundary.sh

  say "embedded dependency boundary"
  bash scripts/ci/embedded-dependency-boundary.sh
}

case "${STAGE}" in
  lint) stage_lint ;;
  test) stage_test ;;
  gates) stage_gates ;;
  all)
    stage_lint
    stage_test
    stage_gates
    ;;
  *)
    echo "dev loop: unknown stage '${STAGE}' (lint | test | gates | all)" >&2
    exit 2
    ;;
esac

printf '\n== dev loop: %s stage(s) passed ==\n' "${STAGE}"
