#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${1:?usage: advance-marketplace.sh SEMVER|--self-test}"
TAG="v${VERSION}"
WAIT_SECONDS="${MADE_RELEASE_POLL_SECONDS:-15}"
WAIT_ATTEMPTS="${MADE_RELEASE_POLL_ATTEMPTS:-180}"
EXPECTED_PRERELEASE=false
if [[ "${VERSION}" == *-* ]]; then
  EXPECTED_PRERELEASE=true
fi

CRATES=(
  made-core made-api made-proto made-client made-console made-app
  made-adapters made-embedded made-mcp-proto made-mcp
)
IMAGES=(
  ghcr.io/underpass-ai/made
  ghcr.io/underpass-ai/made-e2e-runner
  ghcr.io/underpass-ai/made-e2e-stub-runtime
  ghcr.io/underpass-ai/made-e2e-stub-llm
)
SHORT_SHA="$(git rev-parse --short=7 HEAD)"

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
        if json.loads(line).get("vers") == version:
            found = True
    except json.JSONDecodeError:
        pass
raise SystemExit(0 if found else 1)
' "${version}"
}

crate_is_public() {
  local crate="$1"
  curl -fsS -H 'Cache-Control: no-cache' \
    "https://index.crates.io/$(crate_index_path "${crate}")" 2>/dev/null \
    | index_body_has_version "${VERSION}"
}

if [[ "${VERSION}" == "--self-test" ]]; then
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
    echo "marketplace self-test accepted the wrong sparse-index version" >&2
    exit 1
  fi
  echo "marketplace self-test passed: index input is drained and versions are exact"
  exit 0
fi

cd "${ROOT_DIR}"
command -v gh >/dev/null 2>&1 || {
  echo "error: gh is required to verify public release assets" >&2
  exit 127
}
for command in curl docker helm; do
  command -v "${command}" >/dev/null 2>&1 || {
    echo "error: ${command} is required to verify the complete public release" >&2
    exit 127
  }
done

SCRATCH="$(mktemp -d)"
trap 'rm -rf "${SCRATCH}"' EXIT
python3 scripts/ci/made-marketplace-contract.py --require-release-tag --print-assets \
  >"${SCRATCH}/expected.txt"

manifest_digest() {
  docker buildx imagetools inspect "$1" --format '{{json .Manifest}}' 2>/dev/null \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["digest"])' 2>/dev/null
}

release_assets_ready() {
  if ! gh release view "${TAG}" --json isDraft,isPrerelease,assets \
    >"${SCRATCH}/release.json" 2>/dev/null; then
    return 1
  fi
  if ! python3 - "${EXPECTED_PRERELEASE}" "${SCRATCH}/release.json" \
    >"${SCRATCH}/published.txt" <<'PY'
import json
import pathlib
import sys

expected_prerelease = sys.argv[1] == "true"
release = json.loads(pathlib.Path(sys.argv[2]).read_text())
if release["isDraft"] or release["isPrerelease"] is not expected_prerelease:
    raise SystemExit(1)
for asset in sorted({item["name"] for item in release["assets"]}):
    print(asset)
PY
  then
    return 1
  fi
  cmp -s "${SCRATCH}/expected.txt" "${SCRATCH}/published.txt"
}

crates_ready() {
  local crate
  for crate in "${CRATES[@]}"; do
    crate_is_public "${crate}" || return 1
  done
}

images_ready() {
  local image commit_digest release_digest
  for image in "${IMAGES[@]}"; do
    commit_digest="$(manifest_digest "${image}:sha-${SHORT_SHA}")" || return 1
    release_digest="$(manifest_digest "${image}:${TAG}")" || return 1
    [[ -n "${commit_digest}" && "${commit_digest}" == "${release_digest}" ]] || return 1
  done
}

chart_ready() {
  if ! helm show chart oci://ghcr.io/underpass-ai/charts/made \
    --version "${VERSION}" >"${SCRATCH}/chart.yaml" 2>/dev/null; then
    return 1
  fi
  grep -Fxq "version: ${VERSION}" "${SCRATCH}/chart.yaml" \
    && grep -Fxq "appVersion: ${VERSION}" "${SCRATCH}/chart.yaml"
}

for ((attempt = 1; attempt <= WAIT_ATTEMPTS; attempt++)); do
  if release_assets_ready && crates_ready && images_ready && chart_ready; then
      echo "release assets: all $(wc -l <"${SCRATCH}/published.txt") are public and immutable"
      echo "distribution: ${#CRATES[@]} crates, ${#IMAGES[@]} images and Helm ${VERSION} are public"
      if [[ "${EXPECTED_PRERELEASE}" == "true" ]]; then
        echo "${TAG} is a prerelease; stable marketplace remains unchanged"
        exit 0
      fi
      HEAD_COMMIT="$(git rev-parse HEAD)"
      git push origin "${HEAD_COMMIT}:refs/heads/marketplace"
      echo "${TAG} is public and marketplace now serves ${HEAD_COMMIT}"
      exit 0
  fi

  if [[ "${attempt}" -eq 1 ]]; then
    echo "waiting for ${TAG} assets, crates, commit-image promotion and Helm chart..."
  fi
  sleep "${WAIT_SECONDS}"
done

echo "error: ${TAG} did not publish the complete immutable distribution in time" >&2
comm -23 "${SCRATCH}/expected.txt" "${SCRATCH}/published.txt" 2>/dev/null \
  | sed 's/^/missing: /' >&2 || true
echo "marketplace stays at its previous commit" >&2
exit 1
