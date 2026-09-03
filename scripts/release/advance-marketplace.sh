#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${1:?usage: advance-marketplace.sh X.Y.Z}"
TAG="v${VERSION}"
WAIT_SECONDS="${MADE_RELEASE_POLL_SECONDS:-15}"
WAIT_ATTEMPTS="${MADE_RELEASE_POLL_ATTEMPTS:-180}"

cd "${ROOT_DIR}"
command -v gh >/dev/null 2>&1 || {
  echo "error: gh is required to verify public release assets" >&2
  exit 127
}

SCRATCH="$(mktemp -d)"
trap 'rm -rf "${SCRATCH}"' EXIT
python3 scripts/ci/made-marketplace-contract.py --print-assets \
  >"${SCRATCH}/expected.txt"

for ((attempt = 1; attempt <= WAIT_ATTEMPTS; attempt++)); do
  if gh release view "${TAG}" \
    --json isDraft,isPrerelease,assets \
    --jq 'select(.isDraft == false and .isPrerelease == false) | .assets[].name' \
    >"${SCRATCH}/published.txt" 2>/dev/null; then
    sort -u "${SCRATCH}/published.txt" -o "${SCRATCH}/published.txt"
    if cmp -s "${SCRATCH}/expected.txt" "${SCRATCH}/published.txt"; then
      echo "release assets: all $(wc -l <"${SCRATCH}/published.txt") are public"
      HEAD_COMMIT="$(git rev-parse HEAD)"
      git push origin "${HEAD_COMMIT}:refs/heads/marketplace"
      echo "${TAG} is public and marketplace now serves ${HEAD_COMMIT}"
      exit 0
    fi
  fi

  if [[ "${attempt}" -eq 1 ]]; then
    echo "waiting for ${TAG} to publish the complete checksummed plugin asset set..."
  fi
  sleep "${WAIT_SECONDS}"
done

echo "error: ${TAG} did not publish the exact expected asset set in time" >&2
comm -23 "${SCRATCH}/expected.txt" "${SCRATCH}/published.txt" 2>/dev/null \
  | sed 's/^/missing: /' >&2 || true
echo "marketplace stays at its previous commit" >&2
exit 1
