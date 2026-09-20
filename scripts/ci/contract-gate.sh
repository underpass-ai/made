#!/usr/bin/env bash
set -euo pipefail

# Contract gate: MADE is API-first. Sync (gRPC / protobuf) and
# async (AsyncAPI) specifications are the source of truth — generated code
# must stay in sync with them, and breaking changes must be detected here
# before any Rust code is built or tested.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

PROTO_DIR="crates/made-proto/proto"
ASYNCAPI_SPEC="specs/asyncapi/made.asyncapi.yaml"
VENDORED_PROTO="crates/made-mcp-proto/proto/underpass/made/v1/made.proto"

echo ">>> [contract-gate] buf format check"
buf format --diff --exit-code "${PROTO_DIR}"

echo ">>> [contract-gate] buf lint (proto)"
buf lint

echo ">>> [contract-gate] buf breaking (proto, against origin/main)"
if git rev-parse --verify origin/main >/dev/null 2>&1; then
  buf breaking --against ".git#branch=origin/main,subdir=${PROTO_DIR}"
else
  echo "::notice::no origin/main reference; skipping breaking check"
fi

echo ">>> [contract-gate] vendored proto is the same contract"
# `made-mcp-proto` vendors the contract so `made-mcp` can be
# published to crates.io with its proto dependency already on the
# registry. Two copies of one contract drift silently: the published
# MCP adapter would speak a wire the server no longer serves, and
# nothing would say so until a call failed in someone else's cluster.
if ! diff -u "${PROTO_DIR}/underpass/made/v1/made.proto" \
  "${VENDORED_PROTO}" >/dev/null; then
  echo "::error::${VENDORED_PROTO} has drifted from ${PROTO_DIR}" >&2
  diff -u "${PROTO_DIR}/underpass/made/v1/made.proto" "${VENDORED_PROTO}" >&2 || true
  echo "fix: cp ${PROTO_DIR}/underpass/made/v1/made.proto ${VENDORED_PROTO}" >&2
  exit 1
fi

echo ">>> [contract-gate] the packaged agentic system example matches the canonical one"
# The engine ships the worked example so a host that has the engine
# has it. Two copies drift silently: whoever reads `api/examples`
# would be reading a document the engine no longer hands out.
CANONICAL_SYSTEM="api/examples/agentic-systems/integrator-delivery-system.json"
PACKAGED_SYSTEM="crates/made-app/src/usecases/agentic_system/examples/integrator-delivery-system.json"
if ! cmp --silent "${CANONICAL_SYSTEM}" "${PACKAGED_SYSTEM}"; then
  echo "::error::${PACKAGED_SYSTEM} has drifted from ${CANONICAL_SYSTEM}" >&2
  diff -u "${CANONICAL_SYSTEM}" "${PACKAGED_SYSTEM}" >&2 || true
  echo "fix: cp ${CANONICAL_SYSTEM} ${PACKAGED_SYSTEM}" >&2
  exit 1
fi

echo ">>> [contract-gate] packaged ceremony fragments match the canonical API examples"
for fragment_name in roundtable_fixed_order broadcast_collect group_chat maker_checker handoff magentic; do
  CANONICAL_FRAGMENT="api/examples/ceremonies/fragments/${fragment_name}.yaml"
  for packaged_fragment in \
    "crates/made-app/src/usecases/fragments/${fragment_name}.yaml" \
    "crates/made-mcp/src/protocol/fragments/${fragment_name}.yaml"; do
    if ! cmp --silent "${CANONICAL_FRAGMENT}" "${packaged_fragment}"; then
      echo "::error::${packaged_fragment} has drifted from ${CANONICAL_FRAGMENT}" >&2
      diff -u "${CANONICAL_FRAGMENT}" "${packaged_fragment}" >&2 || true
      echo "fix: cp ${CANONICAL_FRAGMENT} ${packaged_fragment}" >&2
      exit 1
    fi
  done
done

echo ">>> [contract-gate] protected Compose fixture has every required identity"
python3 scripts/ci/e2e-compose-contract.py --self-test

echo ">>> [contract-gate] asyncapi validate"
asyncapi validate "${ASYNCAPI_SPEC}"

echo ">>> [contract-gate] OK"
