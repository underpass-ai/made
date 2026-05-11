#!/usr/bin/env bash
#
# Install the `choreo-mcp` binary from a Git remote via `cargo install`.
# Supports pinning to a branch, tag, or revision via env vars (mutually
# exclusive). Designed as the canonical end-user install path until
# the crate is published to crates.io.
#
# Usage:
#   bash scripts/mcp/install-choreo-mcp.sh
#   CHOREO_MCP_TAG=v0.1.0 bash scripts/mcp/install-choreo-mcp.sh
#   CHOREO_MCP_BRANCH=main bash scripts/mcp/install-choreo-mcp.sh
#   CHOREO_MCP_REV=<git-sha> bash scripts/mcp/install-choreo-mcp.sh
#
# CARGO_INSTALL_ROOT (optional): change where cargo writes the binary.

set -euo pipefail

GIT_URL="${CHOREO_MCP_GIT_URL:-https://github.com/underpass-ai/underpass-choreographer}"
BRANCH="${CHOREO_MCP_BRANCH:-}"
TAG="${CHOREO_MCP_TAG:-}"
REV="${CHOREO_MCP_REV:-}"

selected_refs=0
[[ -n "${BRANCH}" ]] && selected_refs=$((selected_refs + 1))
[[ -n "${TAG}" ]] && selected_refs=$((selected_refs + 1))
[[ -n "${REV}" ]] && selected_refs=$((selected_refs + 1))

if [[ "${selected_refs}" -gt 1 ]]; then
  echo "set only one of CHOREO_MCP_BRANCH, CHOREO_MCP_TAG, or CHOREO_MCP_REV" >&2
  exit 2
fi

cmd=(cargo install --git "${GIT_URL}" choreo-mcp --locked --force)

if [[ -n "${BRANCH}" ]]; then
  cmd+=(--branch "${BRANCH}")
elif [[ -n "${TAG}" ]]; then
  cmd+=(--tag "${TAG}")
elif [[ -n "${REV}" ]]; then
  cmd+=(--rev "${REV}")
fi

if [[ -n "${CARGO_INSTALL_ROOT:-}" ]]; then
  cmd+=(--root "${CARGO_INSTALL_ROOT}")
fi

"${cmd[@]}"
