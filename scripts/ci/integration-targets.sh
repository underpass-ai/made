#!/usr/bin/env bash
# Container-backed test targets, discovered instead of listed.
#
# A test target runs against real containers when its crate-level attribute
# gates it on a container feature:
#
#     #![cfg(feature = "container-postgres")]
#
# That line is the whole registration. The (package, feature) pair names the
# suite, and each suite is one CI job that boots one kind of container, so
# the split by container kind and the single-threaded startup stay where they
# were. What goes away is the hand-kept list: a new target joins its suite
# the moment it compiles under it, and a target gated on a feature no suite
# runs fails `check` instead of passing unnoticed (issue #225).
#
#   integration-targets.sh check          every gated target belongs to a suite
#   integration-targets.sh args <suite>   the cargo arguments that run a suite
#   integration-targets.sh suites         the suite names
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# suite  package  feature
SUITES=(
  "postgres made-tests-integration container-postgres"
  "nats made-tests-integration container-nats"
  "image made-mcp container-tests"
)

# The only form discovery reads. Anything else that mentions a container
# feature at crate level (`all(...)`, `any(...)`, a second attribute) is
# refused by `check` rather than half-understood.
PLAIN_GATE='^#!\[cfg\(feature = "(container-[a-z0-9-]+)"\)\]$'

die() {
  echo "::error::$*" >&2
  exit 1
}

package_of() {
  sed -n 's/^name = "\(.*\)"$/\1/p' "$1/Cargo.toml" | head -n 1
}

# One line per container-gated target: `package feature target file`.
gated_targets() {
  local manifest crate package file gate target
  for manifest in "${ROOT_DIR}"/crates/*/Cargo.toml; do
    crate="$(dirname "${manifest}")"
    [ -d "${crate}/tests" ] || continue
    package="$(package_of "${crate}")"
    for file in "${crate}"/tests/*.rs; do
      [ -f "${file}" ] || continue
      gate="$(grep -E '^#!\[cfg\(.*container-' "${file}" || true)"
      [ -n "${gate}" ] || continue
      target="$(basename "${file}" .rs)"
      if [[ "$(wc -l <<<"${gate}")" -ne 1 || ! "${gate}" =~ ${PLAIN_GATE} ]]; then
        echo "${package} ! ${target} ${file#"${ROOT_DIR}"/}"
        continue
      fi
      echo "${package} ${BASH_REMATCH[1]} ${target} ${file#"${ROOT_DIR}"/}"
    done
  done
}

suite_of() {
  local package="$1" feature="$2" entry name suite_package suite_feature
  for entry in "${SUITES[@]}"; do
    read -r name suite_package suite_feature <<<"${entry}"
    if [ "${package}" = "${suite_package}" ] && [ "${feature}" = "${suite_feature}" ]; then
      echo "${name}"
      return 0
    fi
  done
  return 1
}

check() {
  local targets package feature target file orphans=()
  targets="$(gated_targets)"
  while read -r package feature target file; do
    [ -n "${package}" ] || continue
    if [ "${feature}" = "!" ]; then
      orphans+=("${file}: gate it with exactly one plain #![cfg(feature = \"container-<kind>\")]")
    elif ! suite_of "${package}" "${feature}" >/dev/null; then
      orphans+=("${file}: no suite runs ${package} --features ${feature}")
    fi
  done <<<"${targets}"
  if [ "${#orphans[@]}" -gt 0 ]; then
    printf '%s\n' "container-backed targets that no CI job would run:" "${orphans[@]/#/  - }" >&2
    die "gate each one on a feature listed in scripts/ci/integration-targets.sh SUITES"
  fi
}

args() {
  local wanted="$1" entry name suite_package suite_feature targets package feature target file
  local found=0 selected=()
  for entry in "${SUITES[@]}"; do
    read -r name suite_package suite_feature <<<"${entry}"
    [ "${name}" = "${wanted}" ] && found=1 && break
  done
  [ "${found}" -eq 1 ] || die "unknown container suite: ${wanted}"
  check
  targets="$(gated_targets)"
  while read -r package feature target file; do
    if [ "${package}" = "${suite_package}" ] && [ "${feature}" = "${suite_feature}" ]; then
      selected+=("${target}")
    fi
  done <<<"${targets}"
  [ "${#selected[@]}" -gt 0 ] || die "container suite ${wanted} discovered no targets"
  echo ">>> ${wanted} suite: ${#selected[@]} targets: ${selected[*]}" >&2
  printf '%s\n' -p "${suite_package}" --features "${suite_feature}"
  for target in "${selected[@]}"; do
    printf '%s\n' --test "${target}"
  done
}

case "${1:-}" in
  check) check ;;
  args) [ $# -eq 2 ] || die "usage: $0 args <suite>"; args "$2" ;;
  suites) for entry in "${SUITES[@]}"; do echo "${entry%% *}"; done ;;
  *) die "usage: $0 check | args <suite> | suites" ;;
esac
