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
  made-client
  made-console
  made-app
  made-adapters
  made-embedded
  made-mcp-proto
  made-mcp
)

: "${PUBLISH_MAX_WAIT_SECS:=1800}"
: "${PUBLISH_INDEX_WAIT_SECS:=900}"
: "${PUBLISH_INDEX_POLL_SECS:=15}"
USER_AGENT="made-release (https://github.com/underpass-ai/made)"

crate_index_path() {
  local crate="$1" length="${#1}"
  case "${length}" in
    1) printf '1/%s' "${crate}" ;;
    2) printf '2/%s' "${crate}" ;;
    3) printf '3/%s/%s' "${crate:0:1}" "${crate}" ;;
    *) printf '%s/%s/%s' "${crate:0:2}" "${crate:2:2}" "${crate}" ;;
  esac
}

index_body_has_version() {
  local version="$1"
  python3 -c '
import json
import sys

version = sys.argv[1]
found = False
for line in sys.stdin:
    try:
        entry = json.loads(line)
    except json.JSONDecodeError:
        continue
    if entry.get("vers") == version:
        found = True
raise SystemExit(0 if found else 1)
' "${version}"
}

fetch_index() {
  local crate="$1"
  curl -fsS -H "User-Agent: ${USER_AGENT}" \
    -H 'Cache-Control: no-cache' \
    "https://index.crates.io/$(crate_index_path "${crate}")"
}

if [[ "${1:-}" == "--self-test" ]]; then
  [[ "$(crate_index_path a)" == "1/a" ]]
  [[ "$(crate_index_path ab)" == "2/ab" ]]
  [[ "$(crate_index_path abc)" == "3/a/abc" ]]
  [[ "$(crate_index_path made-core)" == "ma/de/made-core" ]]
  printf '%s\n' '{"name":"made-core","vers":"0.7.0-rc.1"}' \
    | index_body_has_version 0.7.0-rc.1
  {
    printf '%s\n' '{"name":"made-core","vers":"0.7.0-rc.1"}'
    python3 - <<'PY'
import json
for patch in range(20_000):
    print(json.dumps({"name": "made-core", "vers": f"0.6.{patch}"}))
PY
  } | index_body_has_version 0.7.0-rc.1
  if printf '%s\n' '{"name":"made-core","vers":"0.7.0"}' \
    | index_body_has_version 0.7.0-rc.1; then
    echo "publish crates self-test accepted the wrong sparse-index version" >&2
    exit 1
  fi
  echo "publish crates self-test passed: sparse paths and exact versions"
  exit 0
fi

: "${CARGO_REGISTRY_TOKEN:?CARGO_REGISTRY_TOKEN must be set}"

version_of() {
  cargo metadata --no-deps --format-version 1 \
    | python3 -c "import json,sys; print(next(p['version'] for p in json.load(sys.stdin)['packages'] if p['name']==sys.argv[1]))" "$1"
}

already_published() {
  local crate="$1" version="$2"
  fetch_index "${crate}" 2>/dev/null | index_body_has_version "${version}"
}

wait_until_indexed() {
  local crate="$1" version="$2" waited=0
  while ! already_published "${crate}" "${version}"; do
    if (( waited >= PUBLISH_INDEX_WAIT_SECS )); then
      echo "::error::${crate} ${version} did not reach the sparse index after ${waited}s" >&2
      return 1
    fi
    if (( waited == 0 )); then
      echo "::notice::waiting for ${crate} ${version} to reach the sparse index"
    fi
    sleep "${PUBLISH_INDEX_POLL_SECS}"
    waited=$(( waited + PUBLISH_INDEX_POLL_SECS ))
  done
  echo "indexed ${crate} ${version} after ${waited}s"
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
  else
    echo "::group::cargo publish -p ${crate} (${version})"
    publish_one "${crate}" "${version}"
    echo "::endgroup::"
  fi
  # A successful upload response can precede sparse-index visibility. Do not
  # attempt to publish the dependent crate until Cargo can resolve this one.
  wait_until_indexed "${crate}" "${version}"
done

echo "crate publication complete"
