#!/usr/bin/env bash
# The operator's own path for #192: one script in two roles, one store,
# and one question carried from a supervisor to a working agent.
#
# The test suite proves the same thing against the test binary. This
# proves it against the thing an operator actually runs — the plugin
# launcher — because a capability that only works when a test harness
# starts it is a capability an operator does not have.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PLUGIN_DIR="${ROOT_DIR}/plugins/made"
LAUNCHER="${PLUGIN_DIR}/scripts/run-embedded-mcp.sh"

CEREMONY="plugin-bridge-1"
ITEM="item-1"
EXECUTION="exec-1"
INCARNATION="inc-1"
ROLE="ENGINEER"

mkdir -p "${ROOT_DIR}/tmp"
WORK_DIR="$(mktemp -d "${ROOT_DIR}/tmp/intervention-bridge.XXXXXX")"
trap 'rm -rf "${WORK_DIR}"' EXIT
export MADE_MCP_STORE_PATH="${WORK_DIR}/ceremonies.sqlite3"

cd "${ROOT_DIR}"
bash scripts/plugin/build-local-made-plugin.sh
CI_BINARY="${PLUGIN_DIR}/bin/made-mcp"
[[ -x "${CI_BINARY}" ]] || CI_BINARY="${CI_BINARY}.exe"
# shellcheck source=/dev/null
source "${ROOT_DIR}/tests/plugin/setup-authorization.sh" "${CI_BINARY}"
OWNER="${MADE_AUTH_TRUSTED_HOST_ID}"

# Bootstrapping opens the policy; it grants nothing. Every action this
# path uses is named here, so what the operator is exercising is also
# what the operator authorized.
cat >"${WORK_DIR}/grant.jsonl" <<EOF
{"jsonrpc":"2.0","id":0,"method":"tools/call","params":{"name":"made_issue_authorization_grant","arguments":{"grant_id":"plugin-ci-bridge","grantee_id":"${OWNER}","actions":["publish_ceremony_definition","start_published_ceremony","get_ceremony_instance","claim_ceremony_step","report_ceremony_agent_status","request_ceremony_intervention","respond_to_ceremony_intervention","pull_ceremony_agent_interventions","acknowledge_ceremony_agent_intervention","get_ceremony_intervention","list_ceremony_interventions"],"scope":{"kind":"global"},"valid_from":"2020-01-01T00:00:00Z","delegation_depth":0}}}
EOF
"${LAUNCHER}" <"${WORK_DIR}/grant.jsonl" >"${WORK_DIR}/grant.out"
python3 - "${WORK_DIR}/grant.out" <<'PY_GRANT'
import json, pathlib, sys
response = json.loads(pathlib.Path(sys.argv[1]).read_text().strip().splitlines()[-1])
assert not response.get("error"), response
assert not response["result"].get("isError"), response
PY_GRANT

# Each call is its own launcher process. Naming them is the point: the
# supervisor and the worker never speak to each other, only to the
# store underneath.
supervisor() { "${LAUNCHER}" <"$1"; }
worker() { "${LAUNCHER}" <"$1"; }

fail() {
  echo "MADE intervention bridge: $1" >&2
  exit 1
}

# --- the worker opens the session and claims its step ------------------
cat >"${WORK_DIR}/worker-claim.jsonl" <<EOF
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"made_publish_ceremony_definition","arguments":{"definition_yaml":"version: \"1.0\"\nname: \"plugin_intervention_bridge\"\nstates:\n  - id: OPEN\n    initial: true\n  - id: DONE\n    terminal: true\ntransitions:\n  - from: OPEN\n    to: DONE\n    trigger: finish\nsteps:\n  - id: work\n    state: OPEN\n    handler: embedded_noop\nroles:\n  - id: ENGINEER\n    allowed_actions:\n      - work\n      - finish\n      - respond_to_intervention\n  - id: LEAD\n    allowed_actions:\n      - request_intervention\n      - finish\n"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"made_start_published_ceremony","arguments":{"ceremony_id":"${CEREMONY}","ceremony":"plugin_intervention_bridge","version":"1.0","actor_id":"${OWNER}","actor_kind":"service"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"made_claim_ceremony_step","arguments":{"ceremony_id":"${CEREMONY}","step_id":"work","actor_kind":"agent","lease_owner_id":"${OWNER}","idempotency_key":"plugin-bridge-claim-1","lease_ttl_ms":300000}}}
EOF
worker "${WORK_DIR}/worker-claim.jsonl" >"${WORK_DIR}/worker-claim.out"

CLAIM_FENCE="$(python3 - "${WORK_DIR}/worker-claim.out" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    response = json.loads(line)
    if response.get("id") != 3:
        continue
    result = response["result"]
    assert not result.get("isError"), result
    print(result["structuredContent"]["claim_fence"])
PY
)"
[[ -n "${CLAIM_FENCE}" ]] || fail "the claim did not answer with its fence"

# The engine derives this from the step's coordinates; the operator's
# host reports under the same identity or the claim is not recognised.
OPERATION_ID="$(python3 - "${CEREMONY}" <<'PY'
import hashlib, sys
digest = hashlib.sha256()
digest.update(b"made.execution-operation.v1\x00")
for part in [sys.argv[1].encode(), b"work", (1).to_bytes(4, "big"), (1).to_bytes(4, "big"), (1).to_bytes(4, "big")]:
    digest.update(len(part).to_bytes(8, "big"))
    digest.update(part)
print(digest.hexdigest())
PY
)"

# --- the worker reports itself live ------------------------------------
#
# The roster is process-local: a host reports what it is doing to the
# session it is talking to, and that is where its claim is verified
# when it asks for its questions. So the report and the pull below share
# one launcher process, exactly as a real host's long-lived session
# does. The ledger underneath is durable; the roster is not, and does
# not need to be.
cat >"${WORK_DIR}/worker-status.jsonl" <<EOF
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"made_report_ceremony_agent_status","arguments":{"status":{"ceremony_id":"${CEREMONY}","agent_execution_id":"${EXECUTION}","operation_id":"${OPERATION_ID}","claim_owner_id":"${OWNER}","logical_worker_id":"plugin-bridge-worker","host_agent_id":"${OWNER}","host_agent_incarnation":"${INCARNATION}","role_id":"${ROLE}","step_id":"work","attempt":1,"execution_status":"running","liveness":"fresh","source":"host_report","activity":"working on the step","task_summary":"the bridge fixture's only step","evidence_references":[],"observed_at":"2026-09-20T12:00:00Z","report_sequence":1,"idempotency_key":"plugin-bridge-status-1","claim_fence":"${CLAIM_FENCE}"}}}}
EOF
worker "${WORK_DIR}/worker-status.jsonl" >"${WORK_DIR}/worker-status.out"
python3 - "${WORK_DIR}/worker-status.out" <<'PY_STATUS'
import json, pathlib, sys
response = json.loads(pathlib.Path(sys.argv[1]).read_text().strip().splitlines()[-1])
assert not response["result"].get("isError"), response
PY_STATUS

# --- the supervisor puts a question to that exact agent ----------------
cat >"${WORK_DIR}/supervisor-ask.jsonl" <<EOF
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"made_request_ceremony_intervention","arguments":{"ceremony_id":"${CEREMONY}","intervention_id":"${ITEM}","role_id":"LEAD","role_kind":"human","kind":"opinion","intent":"question","target_agent_execution_id":"${EXECUTION}","target_incarnation":"${INCARNATION}","target_role_id":"${ROLE}","message":"Is the migration still reversible?"}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"made_get_ceremony_intervention","arguments":{"ceremony_id":"${CEREMONY}","intervention_id":"${ITEM}"}}}
EOF
supervisor "${WORK_DIR}/supervisor-ask.jsonl" >"${WORK_DIR}/supervisor-ask.out"

# --- the worker takes it, under a lease --------------------------------
cat >"${WORK_DIR}/worker-pull.jsonl" <<EOF
$(cat "${WORK_DIR}/worker-status.jsonl")
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"made_pull_ceremony_agent_interventions","arguments":{"ceremony_id":"${CEREMONY}","agent_execution_id":"${EXECUTION}","incarnation":"${INCARNATION}","role_id":"${ROLE}"}}}
EOF
worker "${WORK_DIR}/worker-pull.jsonl" >"${WORK_DIR}/worker-pull.out"

read -r DELIVERY_ID LEASE_ID <<<"$(python3 - "${WORK_DIR}/worker-pull.out" <<'PY'
import json, sys
response = json.loads(open(sys.argv[1]).read().strip().splitlines()[-1])
result = response["result"]
assert not result.get("isError"), result
items = result["structuredContent"]["items"]
assert len(items) == 1, items
print(items[0]["delivery_id"], items[0]["lease_id"])
PY
)"
[[ -n "${DELIVERY_ID}" && -n "${LEASE_ID}" ]] || fail "the pull handed over no lease"

# --- the worker says what it saw, then answers -------------------------
cat >"${WORK_DIR}/worker-answer.jsonl" <<EOF
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"made_acknowledge_ceremony_agent_intervention","arguments":{"ceremony_id":"${CEREMONY}","intervention_id":"${ITEM}","delivery_id":"${DELIVERY_ID}","lease_id":"${LEASE_ID}","agent_execution_id":"${EXECUTION}","incarnation":"${INCARNATION}","role_id":"${ROLE}","observation_kind":"received","note":"picked it up","observed_at":"2026-09-20T12:00:05Z"}}}
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"made_acknowledge_ceremony_agent_intervention","arguments":{"ceremony_id":"${CEREMONY}","intervention_id":"${ITEM}","delivery_id":"${DELIVERY_ID}","lease_id":"${LEASE_ID}","agent_execution_id":"${EXECUTION}","incarnation":"${INCARNATION}","role_id":"${ROLE}","observation_kind":"received","note":"picked it up","observed_at":"2026-09-20T12:00:05Z"}}}
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"made_respond_to_ceremony_intervention","arguments":{"ceremony_id":"${CEREMONY}","intervention_id":"${ITEM}","role_id":"${ROLE}","role_kind":"agent","message":"Yes, the migration is still reversible.","delivery_id":"${DELIVERY_ID}","agent_execution_id":"${EXECUTION}","incarnation":"${INCARNATION}"}}}
EOF
worker "${WORK_DIR}/worker-answer.jsonl" >"${WORK_DIR}/worker-answer.out"

# --- the supervisor reads the whole journey ----------------------------
cat >"${WORK_DIR}/supervisor-read.jsonl" <<EOF
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"made_get_ceremony_intervention","arguments":{"ceremony_id":"${CEREMONY}","intervention_id":"${ITEM}"}}}
{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"made_list_ceremony_interventions","arguments":{"ceremony_id":"${CEREMONY}"}}}
EOF
supervisor "${WORK_DIR}/supervisor-read.jsonl" >"${WORK_DIR}/supervisor-read.out"

python3 - "${WORK_DIR}" "${EXECUTION}" "${INCARNATION}" <<'PY'
import json
import pathlib
import sys

work = pathlib.Path(sys.argv[1])
execution, incarnation = sys.argv[2], sys.argv[3]


def responses(name):
    return [
        json.loads(line)
        for line in (work / name).read_text().splitlines()
        if line.strip()
    ]


def ok(name):
    found = responses(name)
    for response in found:
        result = response.get("result", {})
        assert not result.get("isError"), f"{name}: {response}"
    return found


ok("worker-claim.out")
ok("worker-status.out")
asked = ok("supervisor-ask.out")
ok("worker-pull.out")
answered = ok("worker-answer.out")
read = ok("supervisor-read.out")

at_rest = asked[-1]["result"]["structuredContent"]["status_delivery"]
assert at_rest in {"recorded", "queued"}, f"an unoffered item said {at_rest}"

# The repeat is the same fact: a second acknowledgement of the same
# offer must not appear, and it must not have been an error either.
assert len(answered) == 3, answered

final = read[0]["result"]["structuredContent"]
assert final["status_delivery"] == "responded", final
assert final["unresolved"] is False, final
acknowledgements = final["deliveries"]
assert len(acknowledgements) == 1, acknowledgements
assert acknowledgements[0]["incarnation"] == incarnation, acknowledgements
assert acknowledgements[0]["observation_kind"] == "received", acknowledgements
answer = final["responses"][0]
assert answer["executor_agent_execution_id"] == execution, answer
assert answer["executor_incarnation"] == incarnation, answer
assert answer["delivery_id"], answer

listed = read[1]["result"]["structuredContent"]["interventions"]
assert len(listed) == 1, listed

print("recorded -> queued -> delivered -> acknowledged -> responded")
PY

echo "MADE intervention bridge: two roles carried one question to an answer"
