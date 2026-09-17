#!/usr/bin/env bash
# Re-produce `ceremonies.sqlite3`: a ceremony store written by MADE v0.3.1.
#
# The migration this fixture proves is a one-way door, so the store it
# runs on has to be one the old code really wrote — not one this
# repository fabricates in the shape it expects. Everything below is
# driven through `made-mcp` v0.3.1 over stdio, on its embedded SQLite
# backend, exactly as an operator drove it.
#
#   git worktree add /tmp/made-v0.3.1 v0.3.1
#   cd /tmp/made-v0.3.1
#   cargo build -p made-mcp --no-default-features --features embedded --bin made-mcp
#   MADE_MCP_V031=/tmp/made-v0.3.1/target/debug/made-mcp \
#     bash crates/made-tests-integration/fixtures/stores/v0.3.1/produce.sh
#
# The clock is not frozen: the store carries the timestamps of the run
# that made it, which is what a real legacy store carries. Nothing in
# the migration reads them, and the committed file is the one this
# script produced on 2026-09-17.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STORE="${1:-${HERE}/ceremonies.sqlite3}"
BINARY="${MADE_MCP_V031:?set MADE_MCP_V031 to a made-mcp binary built from the v0.3.1 tag}"

rm -f "${STORE}" "${STORE}-wal" "${STORE}-shm"

python3 - "${HERE}/pre_stream_session.yaml" <<'PY' > "${HERE}/.session.jsonl"
import json
import sys

definition = open(sys.argv[1], encoding="utf-8").read()
second_version = definition.replace('version: "1.0"', 'version: "1.1"', 1).replace(
    "Two moves and a terminal state",
    "Two moves and a terminal state, second published version",
    1,
)

COMPLETE = "pre-stream-complete"
MIDFLIGHT = "pre-stream-midflight"

calls = [
    # A published definition, so an imported instance carries a binding
    # that has to survive the import.
    ("made_publish_ceremony_definition", {"definition_yaml": definition}),
    # One session driven to its terminal state.
    ("made_start_published_ceremony", {
        "ceremony": "pre_stream_session", "version": "1.0",
        "ceremony_id": COMPLETE,
        "actor_id": "fixture-operator", "actor_kind": "service",
        "context": {"subject": "where the store goes next"},
    }),
    ("made_claim_ceremony_step", {
        "ceremony_id": COMPLETE, "step_id": "gather",
        "actor_kind": "human", "lease_owner_id": "fixture-host",
        "idempotency_key": "complete-gather-1",
    }),
    ("made_complete_ceremony_step", {
        "ceremony_id": COMPLETE, "step_id": "gather",
        "actor_kind": "human", "status": "completed",
        "output": {"opened": "the subject is on the table"},
    }),
    ("made_apply_ceremony_transition", {
        "ceremony_id": COMPLETE, "trigger": "gathered", "actor_kind": "human",
    }),
    ("made_claim_ceremony_step", {
        "ceremony_id": COMPLETE, "step_id": "settle",
        "actor_kind": "human", "lease_owner_id": "fixture-host",
        "idempotency_key": "complete-settle-1",
    }),
    ("made_complete_ceremony_step", {
        "ceremony_id": COMPLETE, "step_id": "settle",
        "actor_kind": "human", "status": "completed",
        "output": {"decided": "the store migrates copy-on-write"},
    }),
    ("made_apply_ceremony_transition", {
        "ceremony_id": COMPLETE, "trigger": "settled", "actor_kind": "human",
    }),
    # A second session left mid-flight: one step done, the next one
    # pending in the state it belongs to, and one intervention open.
    ("made_start_published_ceremony", {
        "ceremony": "pre_stream_session", "version": "1.0",
        "ceremony_id": MIDFLIGHT,
        "actor_id": "fixture-operator", "actor_kind": "service",
        "context": {"subject": "what the import has to keep"},
    }),
    ("made_claim_ceremony_step", {
        "ceremony_id": MIDFLIGHT, "step_id": "gather",
        "actor_kind": "human", "lease_owner_id": "fixture-host",
        "idempotency_key": "midflight-gather-1",
    }),
    ("made_complete_ceremony_step", {
        "ceremony_id": MIDFLIGHT, "step_id": "gather",
        "actor_kind": "human", "status": "completed",
        "output": {"opened": "half of it is done"},
    }),
    # Moved on, so the session sits in the state whose step is still
    # pending: the next claim after the import is a real next move.
    ("made_apply_ceremony_transition", {
        "ceremony_id": MIDFLIGHT, "trigger": "gathered", "actor_kind": "human",
    }),
    ("made_request_ceremony_intervention", {
        "ceremony_id": MIDFLIGHT, "intervention_id": "still-open",
        "role_id": "FACILITATOR", "role_kind": "human",
        "kind": "opinion",
        "message": "Does the import keep an open item open?",
    }),
    # A second published version of the same definition.
    ("made_publish_ceremony_definition", {"definition_yaml": second_version}),
    ("made_list_ceremony_instances", {}),
]

print(json.dumps({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": {
        "protocolVersion": "2024-11-05", "capabilities": {},
        "clientInfo": {"name": "a7-fixture", "version": "1"},
    },
}))
for index, (tool, arguments) in enumerate(calls, start=2):
    print(json.dumps({
        "jsonrpc": "2.0", "id": index, "method": "tools/call",
        "params": {"name": tool, "arguments": arguments},
    }))
PY

MADE_MCP_BACKEND=embedded MADE_MCP_STORE_PATH="${STORE}" \
  "${BINARY}" < "${HERE}/.session.jsonl" > "${HERE}/.session-out.jsonl" 2>"${HERE}/.session.log"

python3 - "${HERE}/.session-out.jsonl" <<'PY'
import json
import sys

failed = 0
for line in open(sys.argv[1], encoding="utf-8"):
    answer = json.loads(line)
    result = answer.get("result", {})
    if answer.get("error") or result.get("isError"):
        failed += 1
        print(f"call {answer.get('id')} failed: {json.dumps(answer)[:400]}", file=sys.stderr)
if failed:
    raise SystemExit(f"{failed} call(s) failed; the fixture was not produced")
print("every call of the fixture session succeeded")
PY

rm -f "${HERE}/.session.jsonl" "${HERE}/.session-out.jsonl" "${HERE}/.session.log"
rm -f "${STORE}-wal" "${STORE}-shm"
ls -l "${STORE}"
