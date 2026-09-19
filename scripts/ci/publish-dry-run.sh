#!/usr/bin/env bash
#
# Publish dry-run gate for the operator crates and the standalone MCP crates:
#   - `made-client`    (public gRPC client)
#   - `made-console`   (operator CLI)
#   - `made-mcp-proto` (vendored proto crate)
#   - `made-mcp`       (stdio MCP adapter)
#
# Catches packaging regressions (missing metadata, missing files,
# accidental path-only deps) before the publish-distribution
# workflow tries to push to crates.io.
#
# `made-mcp-proto` uses the full `cargo publish --dry-run` flow:
# compiles the staged tarball as a stand-alone crate.
#
# The three downstream crates use `cargo package -l`: `made-client` depends on
# the workspace's `made-proto`, `made-console` on `made-client`, and `made-mcp`
# on `made-mcp-proto`. A first publication cannot resolve those exact versions
# from the registry yet. The real workflow serializes them in dependency order.
# Listing each package still validates metadata, dependency rewriting and the
# staged file set without a registry round-trip.
#
# No CARGO_REGISTRY_TOKEN required — neither command uploads.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

echo "::group::cargo publish --dry-run -p made-mcp-proto"
cargo publish --dry-run -p made-mcp-proto
echo "::endgroup::"

echo "::group::cargo package -l -p made-client"
cargo package --list -p made-client
echo "::endgroup::"

echo "::group::cargo package -l -p made-console"
cargo package --list -p made-console
echo "::endgroup::"

echo "::group::cargo package -l -p made-mcp"
cargo package --list -p made-mcp
echo "::endgroup::"
