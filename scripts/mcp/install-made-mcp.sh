#!/usr/bin/env bash
#
# Install the `made-mcp` binary.
#
# Two modes, controlled by `MADE_MCP_INSTALL_MODE`:
#
#   - `registry` (default): `cargo install made-mcp` from crates.io.
#                           This is the canonical end-user path. The
#                           first registry release lands the next time
#                           a `v*` tag is pushed.
#   - `git`:                `cargo install --git ...` from the source
#                           repository. Supports pinning to a branch,
#                           tag, or revision via env vars (mutually
#                           exclusive). Use this when validating an
#                           unreleased change against a checkout.
#
# Usage:
#   bash scripts/mcp/install-made-mcp.sh                    # registry
#   MADE_MCP_INSTALL_MODE=git    bash scripts/mcp/install-made-mcp.sh
#   MADE_MCP_INSTALL_MODE=git MADE_MCP_TAG=v0.1.0    bash …
#   MADE_MCP_INSTALL_MODE=git MADE_MCP_BRANCH=main   bash …
#   MADE_MCP_INSTALL_MODE=git MADE_MCP_REV=<git-sha> bash …
#
# CARGO_INSTALL_ROOT (optional): change where cargo writes the binary.

set -euo pipefail

MODE="${MADE_MCP_INSTALL_MODE:-registry}"

case "${MODE}" in
  registry)
    cmd=(cargo install made-mcp --locked --force)
    if [[ -n "${MADE_MCP_VERSION:-}" ]]; then
      cmd+=(--version "${MADE_MCP_VERSION}")
    fi
    ;;
  git)
    GIT_URL="${MADE_MCP_GIT_URL:-https://github.com/underpass-ai/made}"
    BRANCH="${MADE_MCP_BRANCH:-}"
    TAG="${MADE_MCP_TAG:-}"
    REV="${MADE_MCP_REV:-}"

    selected_refs=0
    [[ -n "${BRANCH}" ]] && selected_refs=$((selected_refs + 1))
    [[ -n "${TAG}" ]] && selected_refs=$((selected_refs + 1))
    [[ -n "${REV}" ]] && selected_refs=$((selected_refs + 1))

    if [[ "${selected_refs}" -gt 1 ]]; then
      echo "set only one of MADE_MCP_BRANCH, MADE_MCP_TAG, or MADE_MCP_REV" >&2
      exit 2
    fi

    cmd=(cargo install --git "${GIT_URL}" made-mcp --locked --force)

    if [[ -n "${BRANCH}" ]]; then
      cmd+=(--branch "${BRANCH}")
    elif [[ -n "${TAG}" ]]; then
      cmd+=(--tag "${TAG}")
    elif [[ -n "${REV}" ]]; then
      cmd+=(--rev "${REV}")
    fi
    ;;
  *)
    echo "unknown MADE_MCP_INSTALL_MODE: ${MODE} (expected: registry, git)" >&2
    exit 2
    ;;
esac

if [[ -n "${CARGO_INSTALL_ROOT:-}" ]]; then
  cmd+=(--root "${CARGO_INSTALL_ROOT}")
fi

"${cmd[@]}"
