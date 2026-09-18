#!/usr/bin/env bash
set -euo pipefail

# Pull an immutable-by-policy `sha-<commit>` image and prove that the image
# metadata and registry digest identify the requested full commit. The short
# tag locates the main-run image; the full revision label rejects a collision
# or a tag that points at another source tree.

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <full-commit-sha> <image> [<image> ...]" >&2
  exit 2
fi

COMMIT_SHA="$1"
shift
: "${CONTAINER_CLI:=docker}"

if [[ ! "${COMMIT_SHA}" =~ ^[0-9a-f]{40}$ ]]; then
  echo "commit must be a lowercase 40-character SHA-1: ${COMMIT_SHA}" >&2
  exit 2
fi

EXPECTED_TAG="sha-${COMMIT_SHA:0:7}"

for image in "$@"; do
  if [[ "${image}" != *":${EXPECTED_TAG}" ]]; then
    echo "image ${image} is not tagged ${EXPECTED_TAG}" >&2
    exit 2
  fi

  "${CONTAINER_CLI}" pull "${image}"

  revision="$("${CONTAINER_CLI}" image inspect \
    --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}' \
    "${image}")"
  if [[ "${revision}" != "${COMMIT_SHA}" ]]; then
    echo "image ${image} revision ${revision:-<missing>} does not match ${COMMIT_SHA}" >&2
    exit 1
  fi

  digest="$("${CONTAINER_CLI}" image inspect --format '{{ index .RepoDigests 0 }}' "${image}")"
  repository="${image%:*}"
  if [[ "${digest}" != "${repository}@sha256:"* ]]; then
    echo "image ${image} has no registry digest for ${repository}: ${digest:-<missing>}" >&2
    exit 1
  fi
  digest_hash="${digest##*@sha256:}"
  if [[ ! "${digest_hash}" =~ ^[0-9a-f]{64}$ ]]; then
    echo "image ${image} has malformed registry digest: ${digest}" >&2
    exit 1
  fi

  echo "verified image=${image} commit=${COMMIT_SHA} digest=${digest}"
done
