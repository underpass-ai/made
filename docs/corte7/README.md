# The C7 capabilities, by the operator's path

Four capabilities landed together: a paused ceremony can hand off to an
auditable successor, a question can be put to the agent that is actually
working and be known to have arrived, several published ceremonies can be
composed into one system with roles and participants, and one host can be put
in charge of a whole scope and driven by what the journal owes it.

Every command below was run over stdio against
`plugins/made/scripts/run-embedded-mcp.sh` — the launcher a host starts, not a
test harness — and the transcripts are linked from the
[acceptance checklists](#acceptance). Nothing here is illustrative.

The reference for each tool is [API and MCP reference](../reference/README.md);
the contract each capability keeps is [runtime](../runtime/README.md). This
page is the route through them.

## Before anything: a store and a grant

The launcher needs a durable store and an authorization policy. Bootstrapping
opens the policy; it grants nothing, so every action a route uses is named in
one explicit grant.

```bash
export MADE_C7_DIR="$(mktemp -d)"
export MADE_MCP_STORE_PATH="${MADE_C7_DIR}/ceremonies.sqlite3"
export MADE_AUTH_POLICY_ID=corte7-e-policy
export MADE_AUTH_TRUSTED_HOST_ID=corte7-e-host
export MADE_CEREMONY_STORE_ID=corte7-e-store
export MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY=$(openssl rand -hex 32)

plugins/made/bin/made-mcp bootstrap-authorization "${MADE_MCP_STORE_PATH}" \
  --policy-id "${MADE_AUTH_POLICY_ID}" \
  --trusted-host-id "${MADE_AUTH_TRUSTED_HOST_ID}"
```

Each route below is a file of JSON-RPC requests, one per line, fed to the
launcher:

```bash
plugins/made/scripts/run-embedded-mcp.sh <requests.jsonl >responses.jsonl
```

The launcher exits when its input ends, so a route that has to read an id out
of one answer before asking the next question is two or more runs against the
same store. That is not a limitation of the examples: it is what a restart
looks like, and three of the four routes below exercise it.

## Ask what this build actually serves

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"made_discover_capabilities","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"made_get_help","arguments":{"audience":"agent"}}}
```

`tools/list` answers **104** tools on this build. `made_discover_capabilities`
groups them, reports which host activation adapter this deployment composed,
and — new in this cut — carries `declared_limits`: what the release will not
do, named beside the capability group it belongs to, with what to do instead.
`made_get_help` with `audience: agent` repeats those limits and the ordered
routes, including the two this cut added: `hand_off_to_a_successor` and
`put_a_question_to_a_working_agent`.

## #187 — hand a paused ceremony to a successor

Publish the definition that turned out to be wrong and the one that replaces
it, run a step under the first, pause, then read the plan before sealing
anything.

```json
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"made_plan_ceremony_successor","arguments":{"ceremony_id":"corte7-origin","definition_name":"corte7_handoff","definition_version":"2.0"}}}
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"made_start_ceremony_successor","arguments":{"ceremony_id":"corte7-origin","plan_id":"handoff-1","definition_name":"corte7_handoff","definition_version":"2.0","carried":["work"],"actor_id":"corte7-e-host","actor_kind":"human"}}}
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"made_resume_ceremony","arguments":{"ceremony_id":"corte7-origin","actor_id":"corte7-e-host","actor_kind":"service"}}}
```

The plan answers with the diff, the whole resume preflight, the evidence a
successor could start from, the disposition every outstanding claim would
require and the blockers; it seals nothing. `plan_id` is the caller's, and it
is what makes the whole operation retryable: the successor's id derives from
it, so an identical repeat verifies rather than duplicates.

The last call is the point: after the handoff is sealed the predecessor is
refused with `lifecycle refused 'superseded_by_successor' while ceremony is
Paused`. It can be cancelled; it can no longer be resumed.

The budget always starts fresh. `transfer_remaining` is spelled in the
contract so a caller asking for it is refused with a reason rather than
quietly given something else — reservations and refunds are held per ceremony,
and a balance moved without its reservation ledger would leave two ceremonies
believing they hold it.

## #192 — put a question to the agent that is working

The whole journey is one script, two launcher roles and one store:

```bash
bash scripts/ci/made-plugin-intervention-bridge.sh
```

It publishes a ceremony, claims its step, reports the working agent's status,
has a supervisor ask a question aimed at that exact execution and incarnation,
pulls it from the agent's side under a lease, acknowledges it, answers it and
reads the status back. It prints the route it proved:

```text
recorded -> queued -> delivered -> acknowledged -> responded
```

Two things are worth knowing before adapting it. `received`, `refused` and
`incapable` are statements about the item and are sealed in the ceremony's
stream; `busy` and `timeout` are statements about the host, count an attempt
and put the offer back. And the live agent roster is process-local: an agent
pulls its own questions from the same server session that reported its status.
What crosses processes is the durable ledger, which is why a question aimed at
an execution is also routed to the seat it holds and stays findable with
`made_list_ceremony_interventions`.

## #203 — compose several ceremonies into one system

Publish the ceremonies first; a system composes them by pin and never owns
them.

```json
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"made_design_agentic_system","arguments":{"design":{"id":"corte7-system","purpose":"show the level above one ceremony","integrator_role_id":"integrator","roles":[{"id":"integrator","responsibility":"keeps the run moving","kind":"integrator"},{"id":"author","responsibility":"writes the draft","kind":"contributor"}],"participants":[{"id":"operator","role":"integrator","kind":"person"},{"id":"writer","role":"author","kind":"agent","binding":{"capabilities":["drafting"]}}],"topology":[{"from":"operator","to":"writer","kind":"coordination","handoff":true}],"ceremonies":[{"id":"drafting","pin":{"name":"corte7_drafting","version":"1.0"},"purpose":"produce the draft","activation":{"kind":"manual"},"role_bindings":{"AUTHOR":"writer"}}]}}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"made_validate_agentic_system","arguments":{"system_id":"corte7-system","revision":1}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"made_publish_agentic_system","arguments":{"system_id":"corte7-system","revision":1}}}
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"made_instantiate_agentic_system","arguments":{"system_id":"corte7-system","revision":1,"execution_id":"corte7-run","inputs":{"drafting":{}},"offers":{"writer":{"specialty":"author","capabilities":["drafting"]}},"actor_id":"corte7-e-host","actor_kind":"service"}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"made_advance_agentic_system_execution","arguments":{"execution_id":"corte7-run","actor_id":"corte7-e-host","actor_kind":"service"}}}
```

`validate` resolves every pin and reports every defect at once, located at the
element it is about; `publish` seals the revision you read and moves the head
to the next one. `instantiate` opens a run offering what the host can actually
supply — a participant nobody offers is recorded unavailable and the
ceremonies needing it are skipped with the reason, never stood in for.
`advance` starts what is now ready and is called again as instances complete.

Authorization for all of it is global. There is no `agentic_system`
authorization scope, so a grant cannot be narrowed to one system or one run:
give the grant to a grantee that only designs and runs systems rather than
widening one a ceremony host already holds. The discovery answer says so under
`declared_limits`.

## #204 — put one host in charge of a whole scope

Publish the definition rather than mounting it inline: an inline definition
belongs to the process that mounted it, and this route deliberately spans
three.

**First run** — grant, publish, start, bind, claim:

```json
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"made_bind_ceremony_integrator","arguments":{"binding_id":"corte7-binding","scope":{"kind":"ceremony","ceremony_id":"corte7-loop"},"role_id":"INTEGRATOR","host_kind":"claude-code","address":"corte7-guide-session","activation":"none","incarnation":"guide-run-1"}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"made_claim_ceremony_step","arguments":{"ceremony_id":"corte7-loop","step_id":"work","actor_kind":"agent","lease_owner_id":"corte7-worker","idempotency_key":"guide-claim-1","lease_ttl_ms":300000}}}
```

Keep the `claim_fence` the claim answers with, and the `binding_id` and
`fence` the bind answers with.

**Second run** — complete the step, then ask what the binding is owed:

```json
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"made_complete_ceremony_step","arguments":{"ceremony_id":"corte7-loop","step_id":"work","claim_fence":"<from the claim>","status":"completed","output":{"note":"done"},"actor_kind":"agent"}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"made_await_integrator_attention","arguments":{"scope":{"kind":"ceremony","ceremony_id":"corte7-loop"},"binding_id":"corte7-binding","incarnation":"guide-run-1","fence":0,"wait_timeout_ms":1500}}}
```

Nobody enqueued anything. The await answers `end_reason: items` with one
`result_available` item carrying its `delivery_id`, a `lease_id`, a thin
context and `loop_state`. It is derived from the journal by a projection the
read path runs for the binding it has just fenced, which is why a process that
was not running when the step completed is still owed it.

**Third run** — say what you are about to do, do it, say it is done:

```json
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"made_acknowledge_integrator_attention","arguments":{"binding_id":"corte7-binding","incarnation":"guide-run-1","fence":0,"delivery_id":"<from the await>","lease_id":"<from the await>","acknowledgement":"intent","action_kind":"integrated","idempotency_key":"guide-finish-1","note":"reading the instance, then proposing the transition"}}}
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"made_acknowledge_integrator_attention","arguments":{"binding_id":"corte7-binding","incarnation":"guide-run-1","fence":0,"delivery_id":"<from the await>","lease_id":"<from the await>","acknowledgement":"processed","action_kind":"integrated","idempotency_key":"guide-finish-1","note":"the result is integrated"}}}
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"made_list_attention_deliveries","arguments":{"binding_id":"corte7-binding"}}}
```

Between the two acknowledgements the documented sequence has the host read the
ceremony again with `made_get_ceremony_instance` — what travelled with the
batch was true when the batch was built — and run the ordinary authorized
command under that same `idempotency_key`. The loop hands out items and records what was
said about them; it performs nothing and authorizes nothing. The list is the
paperwork: the delivery ends at `processed` with the act and the key that
closed it.

Stop when `loop_state` is `completed`, `failed`, `blocked` or
`awaiting_human_decision`. The last two are not failures — they are the loop
saying a person is needed.

Waking a host is a separate question. `made_discover_capabilities` reports the
adapter this deployment composed: with `none` the offer waits in the ledger
and the host follows its scope by asking. Configuring the `command` adapter is
[host activation](../operations/host-activation.md). Over the gRPC-backed MCP
server the reported adapter is always `none`, whatever the service composed —
read it as unknown there, not as configured.

## Acceptance

The evidence lives outside this repository, in the workspace `~/Documents/ai`.
Paths below are relative to that workspace.

### #187 — auditable successor

| What was claimed | Where it was proved |
|:--|:--|
| Plan reports diff, preflight, carried evidence, claim dispositions and blockers, and seals nothing | `artifacts/made/corte7/E/operator-187.responses.jsonl` (id 8) |
| Start seals the handoff and only then opens the successor | `artifacts/made/corte7/E/operator-187.responses.jsonl` (id 9) |
| An identical retry verifies the opening it already made rather than duplicating it | `artifacts/made/corte7/A/journey-out.jsonl` (the request repeated verbatim) |
| A predecessor that sealed a handoff is refused `superseded_by_successor` | `artifacts/made/corte7/E/operator-187.responses.jsonl` (id 10) |
| `transfer_remaining` refused with a reason, and declared in discovery | `docs/adr/019-sealed-plan-then-auditable-successor.md`, `crates/made-mcp/src/guidance/declared_limits.rs` |
| Two tools on all four surfaces, parity row, no historical event byte moved | `artifacts/made/corte7/A/just-check.log`, `artifacts/made/corte7/A/gates-contract.log` |

### #192 — intervention delivery with acknowledgement

| What was claimed | Where it was proved |
|:--|:--|
| A question reaches the exact execution and incarnation and comes back answered, over two launcher processes | `artifacts/made/corte7/E/operator-192-intervention-bridge.log`, `artifacts/made/corte7/B/operator-bridge.log` |
| `delivered` is never reported without a lease or an activation receipt behind it | `artifacts/made/corte7/B/gates-tests.log` |
| Four tools on all four surfaces; `tools/list` counted from the tree | `artifacts/made/corte7/B/tools-list.txt`, `artifacts/made/corte7/B/just-check.log` |
| The delivery ledger's Postgres adapters run the same conformance suite as memory and SQLite | `artifacts/made/corte7/B/gate-sqlite.log`, `artifacts/made/corte7/B/gate-integration.log` |
| The roster is process-local, and it is declared | `crates/made-mcp/src/guidance/declared_limits.rs` |

### #203 — agentic system

| What was claimed | Where it was proved |
|:--|:--|
| Design, validate, publish, instantiate and advance over the launcher | `artifacts/made/corte7/E/operator-203.responses.jsonl`, `artifacts/made/corte7/C/operator-stdio-session.md` |
| Validation resolves every pin and reports every finding at once, located at its element | `artifacts/made/corte7/E/operator-203.responses.jsonl` (id 5) |
| Publish seals the revision read and moves the head to N+1 | `artifacts/made/corte7/E/operator-203.responses.jsonl` (id 6) |
| Revision log under compare-and-swap on all three stores | `artifacts/made/corte7/C/gates-sqlite.log`, `artifacts/made/corte7/C/tests-core-adapters.log` |
| Nine tools on all four surfaces | `artifacts/made/corte7/C/just-check.log` |
| No `agentic_system` authorization scope, declared as a limit | `docs/adr/021-pinned-agentic-system-aggregate.md`, `crates/made-mcp/src/guidance/declared_limits.rs` |

### #204 — integrator loop

| What was claimed | Where it was proved |
|:--|:--|
| Attention is derived from the journal, not enqueued: bind, claim, complete, and the await hands over a real item | `artifacts/made/corte7/E/operator-204b.responses.jsonl` (id 8), `artifacts/made/corte7/D1/surfaces-stdio-session.jsonl` |
| It survives the process: three launcher runs against one store carry one item from derivation to `processed` | `artifacts/made/corte7/E/operator-204{a,b,c}.responses.jsonl` |
| Intent before effect, and the ledger reads back what closed the item | `artifacts/made/corte7/E/operator-204c.responses.jsonl` (ids 9–11) |
| Five tools on all four surfaces; literals recounted from the tree | `artifacts/made/corte7/D1/surfaces-tools-list.txt`, `artifacts/made/corte7/D1/surfaces-just-check.log` |
| The `command` activation adapter woke a real host, and the failure path was exercised for real | `artifacts/made/corte7/D2/host-activation-claude-code.md`, `artifacts/made/corte7/D2/host-activation-codex.md` |
| The whole loop end to end against a woken host, over two processes, one durable store and three kills | `artifacts/made/corte7/D2/loop-evidence.jsonl`, `artifacts/made/corte7/D2/loop-just-check.log` |
| Host activation is not reported over gRPC, and it is declared | `crates/made-mcp/src/guidance/declared_limits.rs` |

### Still declared as limits

`AttentionKind::DeadlineExceeded` has no producer; `InactivityDetected` is not
produced; a ceremony-scope binding cannot carry an attention policy of its own
and uses the defaults; the request and the answer to a human guard share one
`AttentionKind` and are told apart only by their reason; the `blocked` reason —
`no_progress` against `round_limit` — is in the evidence and in tracing but on
no surface; and stall detection is taken from a bounded walk of one
binding's ledger — twenty pages of a hundred — so past two thousand records the
count of what a binding has closed saturates and can fall as older records leave
the window, which reads as movement and stops `no_progress` firing on a
long-lived binding. None of them is presented as delivered anywhere.
