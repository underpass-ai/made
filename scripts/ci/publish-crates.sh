#!/usr/bin/env bash
set -euo pipefail

# Publish the workspace's public crates to crates.io, in dependency order.
#
# Order is not a preference: cargo refuses to upload a crate whose
# requirements it cannot resolve on the registry, so a dependency that is
# not there yet fails the whole release. The list below is the transitive
# closure needed by `made-mcp`, deepest first.
#
# Two properties this script guarantees, both learned the hard way:
#
#   * Idempotence. A version already on the registry is skipped rather
#     than retried, because crates.io refuses a re-upload and a release
#     that half-published must be resumable by re-running the job.
#   * Patience. crates.io allows a burst of new crates and then throttles
#     to one every ten minutes. A first release publishes more crates than
#     that burst allows, so a 429 is an expected state, not a failure.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

CRATES=(
  made-core
  made-api
  made-proto
  made-app
  made-adapters
  made-embedded
  made-mcp-proto
  made-mcp
)

: "${CARGO_REGISTRY_TOKEN:?CARGO_REGISTRY_TOKEN must be set}"
: "${PUBLISH_MAX_WAIT_SECS:=1800}"
USER_AGENT="made-release (https://github.com/underpass-ai/made)"

version_of() {
  cargo metadata --no-deps --format-version 1 \
    | python3 -c "import json,sys; print(next(p['version'] for p in json.load(sys.stdin)['packages'] if p['name']==sys.argv[1]))" "$1"
}

already_published() {
  local crate="$1" version="$2" body
  body="$(curl -sS -H "User-Agent: ${USER_AGENT}" \
    "https://crates.io/api/v1/crates/${crate}/${version}" || true)"
  [[ "${body}" == *"\"num\":\"${version}\""* ]]
}

publish_one() {
  local crate="$1" version="$2" waited=0 delay=60 output status

  while :; do
    set +e
    # Verification stays on: it builds the packaged tarball, which is the
    # only thing that catches a file the `include` list forgot — the way a
    # missing README and a missing .proto both look until someone consumes
    # the crate.
    output="$(cargo publish -p "${crate}" 2>&1)"
    status=$?
    set -e
    if [[ ${status} -eq 0 ]]; then
      echo "published ${crate} ${version}"
      return 0
    fi
    # Losing the race with our own earlier attempt is success, not failure.
    if grep -qi "already .*uploaded\|already exists" <<<"${output}"; then
      echo "${crate} ${version} was already on the registry"
      return 0
    fi
    if ! grep -qi "429\|too many requests\|rate limit" <<<"${output}"; then
      echo "${output}" >&2
      return 1
    fi
    if (( waited >= PUBLISH_MAX_WAIT_SECS )); then
      echo "::error::rate limited for ${waited}s publishing ${crate}; giving up" >&2
      echo "${output}" >&2
      return 1
    fi
    echo "::notice::crates.io rate limit hit on ${crate}; retrying in ${delay}s"
    sleep "${delay}"
    waited=$(( waited + delay ))
    delay=$(( delay < 600 ? delay * 2 : 600 ))
  done
}

for crate in "${CRATES[@]}"; do
  version="$(version_of "${crate}")"
  if already_published "${crate}" "${version}"; then
    echo "skip ${crate} ${version}: already on crates.io"
    continue
  fi
  echo "::group::cargo publish -p ${crate} (${version})"
  publish_one "${crate}" "${version}"
  echo "::endgroup::"
done

echo "crate publication complete"
