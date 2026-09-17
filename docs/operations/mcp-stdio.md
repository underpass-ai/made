# MCP Stdio Adapter

Status: installable stdio adapter for the MADE gRPC API.

The repo ships a stdio MCP server in
[`crates/made-mcp`](../../crates/made-mcp). It exposes every RPC of
`underpass.made.v1` as an MCP tool, so coding agents (Codex CLI,
Claude Desktop) can drive MADE without re-implementing
gRPC.

Companion docs:

- [Codex CLI configuration](./mcp/codex.md)
- [Claude Desktop configuration](./mcp/claude-desktop.md)

## Quickstart — fixture mode

After installing `made-mcp`, the fastest client-wiring check needs
no running MADE and no gRPC endpoint:

```bash
MADE_MCP_BACKEND=fixture made-mcp
```

That starts the stdio MCP server and waits for JSON-RPC on stdin. For
a terminal smoke that exits immediately:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | MADE_MCP_BACKEND=fixture made-mcp
```

From a checkout, without installing the binary first:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | MADE_MCP_BACKEND=fixture cargo run -q -p made-mcp --locked
```

Fixture mode returns deterministic canned responses for every tool. It
is for MCP client setup, tool-choice validation, and demos; it is not a
live MADE integration test.

## Quickstart — live local gRPC

To test the MCP adapter against a real local MADE, use two
terminals.

Terminal 1 starts MADE with no external services and seeds one
demo council:

```bash
MADE_NATS_ENABLED=false MADE_SEED_SPECIALTIES=triage just run
```

If `just` is not installed, use the equivalent Cargo command:

```bash
MADE_NATS_ENABLED=false MADE_SEED_SPECIALTIES=triage \
  cargo run --locked -p made
```

Terminal 2 starts the MCP stdio adapter against the local gRPC endpoint:

```bash
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 made-mcp
```

For a one-shot terminal smoke from a checkout:

```bash
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 \
MADE_MCP_BIN=target/debug/made-mcp \
  bash scripts/mcp/made-stdio-smoke.sh
```

The smoke calls `made_list_councils` and expects the seeded `triage`
council. If `made-mcp` is already installed on PATH, omit
`MADE_MCP_BIN`.

## Tool Call Examples

### CreateCouncil

`made_create_council` creates a council for a specialty and asks the
server to seat `num_agents` agents. In live mode those agents must
already be resolvable. The gRPC handler mints ids in the form
`agent-<specialty>-<index>`, so `{"specialty":"triage","num_agents":1}`
expects `agent-triage-0` to exist.

Fixture-mode terminal check:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"made_create_council","arguments":{"specialty":"triage","num_agents":1}}}' \
  | MADE_MCP_BACKEND=fixture cargo run -q -p made-mcp --locked
```

Expected response shape:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "isError": false,
    "structuredContent": {
      "council": {
        "specialty": "triage",
        "num_agents": 1,
        "agents": []
      }
    }
  }
}
```

The fixture response is deterministic and does not mutate state. For a
live local call, first ensure the matching agent exists through seeding
or `made_register_agent`, then set
`MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055` instead of
`MADE_MCP_BACKEND=fixture`.

### RegisterAgent

`made_register_agent` registers an agent descriptor so later calls can
resolve that agent by id. It does not attach the agent to a council by
itself; `CreateCouncil` still controls council membership.

Fixture-mode terminal check:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"made_register_agent","arguments":{"specialty":"review","agent":{"agent_id":"agent-review-0","specialty":"review","kind":"noop"},"agent_config":{"label":"local noop reviewer"}}}}' \
  | MADE_MCP_BACKEND=fixture cargo run -q -p made-mcp --locked
```

Expected response shape:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "isError": false,
    "structuredContent": {
      "agent_id": "agent-fixture-1"
    }
  }
}
```

For a live local call, use `kind: "noop"` when you want a provider-free
agent. Provider-backed kinds such as `openai` or `vllm` require the
corresponding adapter and environment to be configured. Per-agent
factory options belong in top-level `agent_config`; the nested `agent`
object is only the public summary (`agent_id`, `specialty`, `kind`,
optional `attributes`). If the next step is `CreateCouncil`, keep the id
pattern `agent-<specialty>-<index>`; for the example above that means
creating a `review` council with `num_agents: 1`.

### RegisterContract

`made_register_contract` stores an `OutputContract` in the contract
registry. Later `RunCouncilDecision` calls reference it by
`contract_id` and validate the council winner against its field rules
and optional embedded JSON Schema.

Fixture-mode terminal check:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"made_register_contract","arguments":{"contract":{"contract_id":"contract-review-v1","format":"json_object","fields":{"status":{"required":true,"allowed_string_values":["accepted","needs_changes"]},"summary":{"required":true},"rationale":{"required":false}}}}}}' \
  | MADE_MCP_BACKEND=fixture cargo run -q -p made-mcp --locked
```

Expected response shape:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "isError": false,
    "structuredContent": {
      "contract_id": "contract-fixture-1"
    }
  }
}
```

For a live local call, keep the returned or requested `contract_id` and
pass it to `made_run_council_decision`. `format` is currently
`json_object`. Field rules can require named fields and constrain string
values; for stricter validation, include a `json_schema` string. The
canonical Report-shape example lives at
[`api/examples/output-contracts/report.schema.json`](../../api/examples/output-contracts/report.schema.json).

### RunCouncilDecision

`made_run_council_decision` runs a council and validates the winning
proposal against a previously registered contract. The call must include
`contract_id`, `description`, and exactly one selector:
`specialty` or `council_id`.

Fixture-mode terminal check:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"made_run_council_decision","arguments":{"specialty":"review","contract_id":"contract-review-v1","description":"Review the candidate change and return status, summary, and rationale.","validation_mode":"VALIDATION_MODE_STRICT"}}}' \
  | MADE_MCP_BACKEND=fixture cargo run -q -p made-mcp --locked
```

Expected response shape:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "isError": false,
    "structuredContent": {
      "task_id": "task-fixture-1",
      "winner": {
        "rank": 0,
        "proposal": {
          "proposal_id": "proposal-fixture-a",
          "author_agent_id": "agent-fixture-1",
          "content": "fixture answer",
          "metadata": {},
          "revision_count": 0
        },
        "validation": {
          "score": 1.0,
          "reports": [
            {
              "kind": "content-non-empty",
              "passed": true,
              "summary": "ok",
              "details": {}
            }
          ]
        }
      },
      "validation": {
        "passed": true,
        "candidates_passed": 1,
        "candidates_total": 1
      },
      "candidates": [
        {
          "proposal_id": "proposal-fixture-a",
          "author_agent_id": "agent-fixture-1",
          "score": 1.0,
          "reports": [
            {
              "kind": "content-non-empty",
              "passed": true,
              "summary": "ok",
              "details": {}
            }
          ],
          "rank": 0,
          "passed": true,
          "revision_count": 0
        }
      ],
      "duration_ms": 42,
      "validation_mode": "VALIDATION_MODE_STRICT"
    }
  }
}
```

For a live local call, the selected council must exist and the
`contract_id` must already be registered. `VALIDATION_MODE_STRICT`
fails the call when no candidate satisfies the contract; use
`VALIDATION_MODE_WARN` when the caller wants the best-ranked candidate
returned even if validation fails.

### Orchestrate

`made_orchestrate` runs the full path: deliberate on the task's
specialty, pick the winning proposal, and pass that winner to the
configured `ExecutorPort`. The call takes a `task` object and optional
opaque `execution_options`.

Fixture-mode terminal check:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"made_orchestrate","arguments":{"task":{"task_id":"task-review-orchestrate-1","description":"Review the candidate change and execute the accepted plan.","specialty":"review","constraints":{"rounds":1,"num_agents":1}},"execution_options":{"executor":"noop","trace_label":"mcp-orchestrate-demo"}}}}' \
  | MADE_MCP_BACKEND=fixture cargo run -q -p made-mcp --locked
```

Expected response shape:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "isError": false,
    "structuredContent": {
      "task_id": "task-fixture-1",
      "execution_id": "exec-fixture-1",
      "duration_ms": 73,
      "winner": {
        "rank": 0,
        "proposal": {
          "proposal_id": "proposal-fixture-a",
          "author_agent_id": "agent-fixture-1",
          "content": "fixture answer",
          "metadata": {},
          "revision_count": 0
        },
        "validation": {
          "score": 1.0,
          "reports": [
            {
              "kind": "content-non-empty",
              "passed": true,
              "summary": "ok",
              "details": {}
            }
          ]
        }
      },
      "candidates": [],
      "metadata": {
        "fixture": true
      }
    }
  }
}
```

For a live local call, `task.specialty` must point to an existing
council. The default local executor is `noop`; set the Runtime executor
environment only when you want the winner sent to an external Runtime
service. `execution_options` is forwarded to the configured executor and
takes precedence over overlapping execution-profile metadata.

## Tools

### When a tool call fails

A failed tool call is an MCP tool **result** with `"isError": true`, not
a JSON-RPC `error`: the JSON-RPC layer only fails when the message
itself is unusable. Every backend answers the same envelope, in the
result's `structuredContent`:

```json
{
  "content": [{ "type": "text", "text": "not_found: not found: ceremony_instance" }],
  "structuredContent": {
    "code": "not_found",
    "message": "not found: ceremony_instance",
    "retryable": false
  },
  "isError": true
}
```

`code` is one of five words, and each names a different remedy:

| `code` | What happened | `retryable` |
|---|---|---|
| `unavailable` | The engine was not reached. | `true` |
| `not_found` | What the call named is not there. | `false` |
| `conflict` | Somebody else wrote to what this call was writing to. | `true` |
| `refused` | The engine looked at the call and said no. | `false` |
| `invalid_request` | The arguments do not fit the tool's schema. | `false` |

Branch on `code`, never on the message: the message is the engine's own
words and may change. The same failure carries the same code whichever
backend served it, which is what lets a client point at an in-process
engine or a cluster without a second error table. `retryable` is `true`
for the two failures that can change without the caller changing
anything — an engine that was not reached, and a race that was lost —
and the remedy for `conflict` in particular is to read the session again
and repeat the call. The other three answer the same way however many
times they are asked.

The text content carries `code: message` for hosts that render nothing
else. The transport never appears in either: `unavailable` says the same
thing whether the engine was across a network or failed to open in this
process.

### Discovering the active surface and getting help

Two server-owned tools are available independently of the selected backend:

- `made_discover_capabilities` returns the server name and version, active
  backend and TLS posture, backend-filtered tools, capability groups and
  artifact generators. It is the machine-readable source for deciding what
  this running process can execute.
- `made_get_help` accepts `audience: user` or `audience: agent`. User help
  presents available workflows and examples. Agent help adds preconditions,
  authority boundaries, delegated-host sequencing and explicit responses to
  protocol errors, tool errors, absent tools and lost host context.

```json
{"name":"made_discover_capabilities","arguments":{}}
```

```json
{"name":"made_get_help","arguments":{"audience":"agent"}}
```

Both responses are derived against the same catalog filter as `tools/list`:
a backend that cannot execute a tool neither lists it nor recommends its
workflow. Every ceremony tool now has an RPC behind it, so what the gRPC
backend filters out is nothing, and what the embedded backend filters out is
the council surface. Status and metrics are served by both (`service_observability`).

The 41 backend-owned MCP tools are 1:1 with MADE's 41 gRPC RPCs.
Together with the two server-owned discovery/help tools above, gRPC mode
advertises 43 executable tools:

| MCP tool                          | gRPC RPC                              | Purpose |
|-----------------------------------|---------------------------------------|---------|
| `made_deliberate`               | `Deliberate`                          | Run a council deliberation; returns ranked proposals. |
| `made_stream_deliberation`      | `StreamDeliberation`                  | Same as above but every phase-transition frame buffered into one response (stdio is sync). |
| `made_get_deliberation_result`  | `GetDeliberationResult`               | Fetch a previously-executed deliberation by task id. |
| `made_orchestrate`              | `Orchestrate`                         | Deliberate AND execute the winner through the wired executor. |
| `made_create_council`           | `CreateCouncil`                       | Create / replace a council for a specialty. |
| `made_list_councils`            | `ListCouncils`                        | Enumerate registered councils. |
| `made_delete_council`           | `DeleteCouncil`                       | Idempotent delete. |
| `made_register_agent`           | `RegisterAgent`                       | Register an agent descriptor (`noop` / `anthropic` / `openai` / `vllm`). |
| `made_unregister_agent`         | `UnregisterAgent`                     | Remove an agent. |
| `made_process_trigger_event`    | `ProcessTriggerEvent`                 | Submit a domain event; fans out to deliberations. |
| `made_run_council_decision`     | `RunCouncilDecision`                  | Run a council against a registered output contract; returns the validated winner plus per-candidate breakdown. |
| `made_register_contract`        | `RegisterContract`                    | Register an `OutputContract` in the contract registry. |
| `made_list_contracts`           | `ListContracts`                       | Enumerate registered contracts. |
| `made_delete_contract`          | `DeleteContract`                      | Idempotent contract delete. |
| `made_run_ceremony`             | `RunCeremony`                         | Execute a declarative ceremony YAML; returns final state, per-step winning contributions, and the Mermaid conversation diagram. |
| `made_get_ceremony_instance`    | `GetCeremonyInstance`                 | Inspect one persistent ceremony instance. |
| `made_list_ceremony_instances`  | `ListCeremonyInstances`               | Discover persistent ceremony instances. |
| `made_start_ceremony`           | `StartCeremony`                       | Start supplied YAML without advancing. |
| `made_start_published_ceremony` | `StartPublishedCeremony`              | Start an immutable published definition. |
| `made_run_ceremony_step`        | `RunCeremonyStep`                     | Invoke the configured server-owned step handler. |
| `made_apply_ceremony_transition` | `ApplyCeremonyTransition`            | Apply an enabled transition. |
| `made_approve_ceremony_guard`   | `ApproveCeremonyGuard`                | Record an explicit human guard approval. |
| `made_defer_ceremony_guard`     | `DeferCeremonyGuard`                  | Preserve a human deferral. |
| `made_request_ceremony_intervention` | `RequestCeremonyIntervention`    | Open a participant request. |
| `made_respond_to_ceremony_intervention` | `RespondToCeremonyIntervention` | Record a targeted response. |
| `made_close_ceremony_intervention` | `CloseCeremonyIntervention`        | Close a participant request. |
| `made_collect_ceremony_evidence` | `CollectCeremonyEvidence`            | Attach evidence from a configured source. |
| `made_assert_ceremony_reason`   | `AssertCeremonyReason`                | Record a participant-attributed reason. |
| `made_validate_ceremony_draft`  | `ValidateCeremonyDraft`               | Validate without publishing. |
| `made_explain_ceremony_draft`   | `ExplainCeremonyDraft`                | Explain structure and findings. |
| `made_publish_ceremony_definition` | `PublishCeremonyDefinition`        | Publish an immutable definition. |
| `made_diff_ceremony_definitions` | `DiffCeremonyDefinitions`            | Compare two definitions. |
| `made_bind_ceremony_participants` | `BindCeremonyParticipants`          | Seat participants in declared roles. |
| `made_claim_ceremony_step`      | `ClaimCeremonyStep`                   | Lease one step the host will execute itself. |
| `made_complete_ceremony_step`   | `CompleteCeremonyStep`                | Record the observable result of a claimed host-executed step. |
| `made_design_ceremony`          | `DesignCeremony`                      | Turn an author's structured intent into an unpublished ceremony draft. |
| `made_read_ceremony_events`     | `ReadCeremonyEvents`                  | Read the sealed event stream of one session by position, with its hash chain. |
| `made_get_ceremony_transcript`  | `GetCeremonyTranscript`               | Read what the completed steps of one session contributed. |
| `made_generate_ceremony_report` | `GenerateCeremonyReport`              | Render the Markdown report of one session from its persisted state. |
| `made_get_status`               | `GetStatus`                           | Service health, version, uptime, optional stats. |
| `made_get_metrics`              | `GetMetrics`                          | Statistics snapshot. |
| `made_verify_ceremony_journal`  | `VerifyCeremonyJournal`               | Verify the hash chain of one ceremony's journal. |

The MADE API is **respected at 100%** — every proto field has
an explicit JSON key in both the tool input schema and the response.
No flattening, no silent drops. Enums (e.g. `DeliberationPhase`) map
to stable string labels (`DELIBERATION_PHASE_PROPOSING`, …).

### Reading what a session left behind

`made_read_ceremony_events` hands out the **sealed records** of one ceremony's
event stream, in order: the fact, its position, its actor and timestamps, the
correlation and causation ids, the payload the digest covers, and the hash
chain. A client can read the answer back into a record and verify the chain
itself rather than trusting the server that sent it.

Reading is by position. `from_version` is the version already seen — omit it,
or send `0`, to read from the first record — and the answer carries
`next_version` to send back for the next page plus `head_version`, so a caller
can tell "there is more" from "you are caught up" without asking again. An
omitted `limit` takes 200 records and 1000 is the cap. A ceremony with no
stream is `not_found`.

```json
{
  "name": "made_read_ceremony_events",
  "arguments": { "ceremony_id": "session-17", "from_version": 12, "limit": 50 }
}
```

`made_get_ceremony_transcript` hands out the ordered contributions the session's
steps produced — `step_id`, `role_id` and the structured output. It is folded
from the `step_completed` records of the stream above, so it is exactly as
durable as the session and holds every step that completed, whether the engine
ran it or a host claimed it and reported back. A ceremony with no stream is
`not_found`, the way reading its events is.

`made_verify_ceremony_journal` answers whether one ceremony's journal is
sealed, positioned and linked as it was written. The answer names the head
version, how many records were verified, `intact`, and — when it is not — the
first position that cannot be trusted and why, in words. It stops at the first
defect: past a break the verifier does not know what it is looking at, so a
list of further ones would suggest otherwise. A ceremony with no stream is
`not_found`, and a broken chain is an answer rather than an error.

```json
{
  "name": "made_verify_ceremony_journal",
  "arguments": { "ceremony_id": "session-17" }
}
```

```json
{
  "ceremony_id": "session-17",
  "head_version": 12,
  "record_count": 12,
  "intact": true,
  "first_broken_sequence": null,
  "reason": null
}
```

The engine's verdict is checkable rather than authoritative: the same records
come out of `made_read_ceremony_events`, and the verifier is
`AuditChain::verify` in `made-core`, which depends on nothing but the bytes it
was given. A caller that would rather not take the engine's word runs it
itself and compares — which is what the integration test does.

All three are served by both editions, over `ReadCeremonyEvents`,
`VerifyCeremonyJournal` and `GetCeremonyTranscript`.

### Ceremony reports

`made_generate_ceremony_report` is a read-only projection of persisted state
(ADR-006), served by **both** editions over the `GenerateCeremonyReport` RPC
and rendered by one `made-app` use case, so the same sessions in the same state
report the same bytes whichever engine answered. A call supplies `ceremony_ids`
as a non-empty array with no duplicates and may supply `title`. Unknown ids
fail the whole call; caller order is preserved.

```json
{
  "name": "made_generate_ceremony_report",
  "arguments": {
    "ceremony_ids": ["session-17", "session-18"],
    "title": "Working-session report"
  }
}
```

The response's `structuredContent` contains `report_markdown`, selected ids,
completed and incomplete counts, and each definition's version plus computed
and bound digests when available. `persisted` is always `false`: the tool does
not create a file. Definition, steps and outputs, transitions, guards and
deferrals, interventions and evidence, reasons, and ordered journal records are
rendered without inferred narrative. Values are not truncated; use smaller id
batches if the MCP client imposes a response-size limit.

## Modes

Backend selection is driven by `MADE_MCP_BACKEND`:

- **`grpc`** (default) — talks to a real MADE. The endpoint
  env var is mandatory; the binary exits with code 2 if it is missing.
- **`embedded`** — executes the real ceremony engine in process, on top of a
  SQLite state file named by `MADE_MCP_STORE_PATH`. That variable is mandatory:
  where ceremony state lives is an operator decision, and the binary exits
  with code 2 rather than inventing a location or quietly running on memory
  that dies with the process. The isolated
  build exposes one-shot execution plus persistent incremental controls for
  starting, inspecting, stepping, claiming/completing host-owned work,
  explicitly approving a human guard, and applying a transition. It can also
  read the sealed event stream and the transcript, and generate deterministic
  Markdown reports from one or more persisted ceremony snapshots and audit
  journals.
  Participants can open, answer, and close dynamic opinion, investigation, or
  action requests while the ceremony remains active. It requires no
  MADE service, gRPC, protobuf, NATS, or database.
- **`fixture`** — returns canned responses for every tool. Useful for
  client wiring, demos, and tool-choice validation **without** a
  running MADE.

```bash
MADE_MCP_BACKEND=fixture cargo run -p made-mcp --locked

MADE_MCP_BACKEND=embedded \
MADE_MCP_STORE_PATH="${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.sqlite3" \
  cargo run -p made-mcp --locked
```

### Sharing one ceremony store between two agent hosts

SQLite is the only embedded storage engine and is enabled in the normal build.
It uses WAL mode, so multiple MCP processes can open the same store: readers do
not block a writer, and concurrent writers serialize at commit. Point every
host at the same `MADE_MCP_STORE_PATH`; do not create one store per host.

### Upgrading an existing Redb store

The current binary neither links Redb nor converts it. It refuses a `.redb`
path or Redb file before modifying it. Stop every process using the old store,
then use the last dual-engine release to perform the one-time conversion:

```bash
cargo install made-mcp --version 0.2.0 --features sqlite --locked \
  --root /tmp/made-mcp-0.2.0
/tmp/made-mcp-0.2.0/bin/made-mcp share-store \
  "${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.redb"
```

Version 0.2.0 snapshots the source, copies and verifies every store table,
installs `ceremonies.sqlite3`, and preserves the original as a backup. Review
the command's receipt, then start the current binary with
`MADE_MCP_STORE_PATH` pointing at that `.sqlite3` file. The new binary has no
engine selector, `convert`, or `share-store` command; those remain available
only in the migration release so the production dependency graph contains no
Redb library or engine.

What survives a restart is bounded by the
[published-definition boundary](./capability-verification.md) — an instance
started from a published definition rehydrates, one started from supplied
YAML keeps its snapshot but cannot reload its definition, and
`made_list_ceremony_instances` reports the latter as
`"rehydratable": false` instead of failing the whole listing. Every
entry of that listing carries `rehydratable` and `reason`, readable or
not, on either backend — test one field rather than inferring
readability from a field that is not there. The full loop is in the
[embedded ceremony execution runbook](./embedded-ceremony-execution.md).

### Who holds a step lease

`made_run_ceremony`, `made_run_ceremony_step` and
`made_claim_ceremony_step` all take an optional `lease_owner_id`. Leave
it out and this MCP server fills it in with `made-mcp:<backend>` —
`made-mcp:embedded` in process, `made-mcp:grpc` against a cluster —
before the call reaches the engine. One omission means one owner, and
which kind of process is holding a lease is readable from the id.

The value is applied here, not left to the engine: a server has its own
default for clients that speak gRPC directly, and from MCP that default
is never reached. Name your own runner whenever the host, and not this
server, is the thing that will come back to finish the step. A blank
`lease_owner_id` is refused rather than defaulted, on both backends: the
field's schema says `minLength: 1`, and a caller who wrote it meant to
name something.

`idempotency_key` and `lease_ttl_ms` are filled in by the same layer,
for the same reason. An omitted key becomes `made-mcp:<uuid>` — the same
prefix on every backend, because the key is **sealed into the journal**
and the evidence of a session must not record which kind of process
happened to answer. An omitted or zero `lease_ttl_ms` becomes 60 000 ms
for `made_run_ceremony`, 30 000 ms for `made_run_ceremony_step` and
300 000 ms for `made_claim_ceremony_step` — the last is longer by an
order of magnitude on purpose, because it covers work a host does where
the engine cannot see progress. Every tool's schema states the number it
applies.

### Numbers in an open payload

`context`, `output`, `details`, `payload`, `attributes` and the
designer's `equals` are objects MADE does not look inside — with one
exception a caller has to know about. The gRPC contract carries them as
`google.protobuf.Struct`, whose numbers are doubles: `{"score": 1}` and
`{"score": 1.0}` are the same eight bytes on the wire and cannot be told
apart again.

So the reading is decided at ingress, identically on every backend:

- A **whole-valued number is read whole.** `1.0` is stored and sealed as
  `1`, so two clients that write the same session leave the same records
  whichever backend they used — and a client that reads the stream back
  and runs its own chain verification gets an intact chain.
- A **number outside ±2^53 is refused** as `invalid_request`. Past that
  range a double no longer counts one at a time, so no surface could
  hand the value back as it was written, and answering with a different
  number would be worse than saying so.
- Everything else — every fraction — is carried exactly as written.

A client that needs `1.0` back as `1.0` should send it as a string.
Carrying the exact bytes on the wire is a contract change and is not in
this phase.

### What the schema decides about your arguments

Every call is checked against the schema the tool publishes, in the
server, before any backend is reached — so the same call is accepted or
refused the same way whichever engine is behind it. Two of those
decisions are worth knowing:

- An **explicit `null` on a field the schema does not require is
  absent.** A host generated from a typed SDK serialises an unset
  optional that way, and it means the same thing as leaving the field
  out.
- A **top-level key starting with `_` is not an argument.** MCP reserves
  `_meta` on the objects it defines, and it is skipped at
  `tools/call.arguments` rather than refused as undeclared. Only there:
  inside an open payload your keys are your own, underscore or not.

### Embedded step execution ownership

There are two distinct execution paths:

- `made_run_ceremony_step` invokes a server-owned step handler. Use it for
  operational work only when the embedding host configured a real
  `CeremonyStepHandlerPort`. The bundled default may use
  `NoopCeremonyStepHandler`; its empty completed result demonstrates protocol
  and state-machine wiring, not that external work occurred.
- For work owned by the MCP host, call `made_claim_ceremony_step` for the
  exact next step, perform the real work through authorized host workers and
  tools, then call `made_complete_ceremony_step` with its observable status,
  structured output, and evidence/artifact references. Refresh the instance
  before applying an enabled transition.

Claiming records a lease and performs no external work. Claim and completion
wire existing application use cases; they grant no new authority and do not
relax human guards or host policy.

Both editions serve this pair. `made_claim_ceremony_step` and
`made_complete_ceremony_step` are backed by the `ClaimCeremonyStep` and
`CompleteCeremonyStep` RPCs, so a host that delegates step execution runs the
same protocol against a cluster as it does in process — the same arguments,
the same answer, the same refusals.

The intervention tools — served by both editions since parity slice F2, over
`RequestCeremonyIntervention`, `RespondToCeremonyIntervention` and
`CloseCeremonyIntervention` — are:

- `made_request_ceremony_intervention`: the requesting role opens a live
  agenda item. Omit `target_role_ids` for the whole table or provide one or
  more role ids for a scoped request.
- `made_respond_to_ceremony_intervention`: a targeted role records one
  response, with optional structured `details`.
- `made_close_ceremony_intervention`: only the requesting role can close
  the item.

The YAML must grant `request_intervention` and `respond_to_intervention` in
the relevant roles' `allowed_actions`. An `action` intervention coordinates a
request; it is not approval to mutate an external system. Host policy and any
human ceremony guards still apply.

## Installation

For users outside the repo, install as a Cargo binary from crates.io
after the first release has published the package:

```bash
cargo install made-mcp --locked
```

The repo helper uses the registry path by default:

```bash
bash scripts/mcp/install-made-mcp.sh
```

For unreleased changes, switch the helper to Git mode and pin a ref:

```bash
MADE_MCP_INSTALL_MODE=git bash scripts/mcp/install-made-mcp.sh

MADE_MCP_INSTALL_MODE=git MADE_MCP_TAG=v0.1.0 bash scripts/mcp/install-made-mcp.sh
MADE_MCP_INSTALL_MODE=git MADE_MCP_REV=<git-sha> bash scripts/mcp/install-made-mcp.sh
```

After install, the adapter is just `made-mcp` on PATH:

```bash
MADE_MCP_GRPC_ENDPOINT=https://made.example.com made-mcp
```

### Distribution model

`made-mcp` depends on `made-mcp-proto`, a small vendored proto
crate that carries only the public `underpass.made.v1` API needed by
the MCP adapter. Release tags publish `made-mcp-proto` first, wait
for crates.io index propagation, and then publish `made-mcp`.

## Live gRPC mode

Plain (no TLS):

```bash
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 \
  cargo run -p made-mcp --locked
```

`https://` endpoints auto-enable server TLS using system / webpki roots:

```bash
MADE_MCP_GRPC_ENDPOINT=https://made.example.com \
  cargo run -p made-mcp --locked
```

Private CAs and direct mTLS are explicit:

```bash
MADE_MCP_GRPC_ENDPOINT=https://made.underpass.svc:50055 \
MADE_MCP_GRPC_TLS_MODE=mutual \
MADE_MCP_GRPC_TLS_CA_PATH=/var/run/made-tls/ca.crt \
MADE_MCP_GRPC_TLS_CERT_PATH=/var/run/made-tls/tls.crt \
MADE_MCP_GRPC_TLS_KEY_PATH=/var/run/made-tls/tls.key \
MADE_MCP_GRPC_TLS_DOMAIN_NAME=made-grpc \
  cargo run -p made-mcp --locked
```

### Env var reference

| Var                              | Purpose                                                                  |
|----------------------------------|--------------------------------------------------------------------------|
| `MADE_MCP_BACKEND`             | `grpc` (default), `embedded`, or `fixture`; the selected backend must be compiled. |
| `MADE_MCP_STORE_PATH`           | state file the embedded backend opens. Required when `BACKEND=embedded`. |
| `MADE_MCP_BIN`                 | plugin launchers only: the executable to run, overriding the bundled binary. |
| `MADE_MCP_GRPC_ENDPOINT`       | URL the MCP connects to. Required when `BACKEND=grpc`.                   |
| `MADE_MCP_GRPC_TLS_MODE`       | `disabled` / `server` / `mutual`. Auto-derived when omitted.             |
| `MADE_MCP_GRPC_TLS_CA_PATH`    | PEM CA bundle. Implies `server` mode when set.                           |
| `MADE_MCP_GRPC_TLS_CERT_PATH`  | Client cert PEM (mutual). Implies `mutual` mode when set.                |
| `MADE_MCP_GRPC_TLS_KEY_PATH`   | Client key PEM (mutual). Implies `mutual` mode when set.                 |
| `MADE_MCP_GRPC_TLS_DOMAIN_NAME`| TLS SNI/domain override when cert CN/SAN differs from the URL host.      |

`RUST_LOG=made_mcp=debug` enables structured per-tool-call tracing
on stderr (stdout is reserved for JSON-RPC). SQLite adapter diagnostics use
the `made_adapters::sqlite` target.

## Smoke test

```bash
# Fixture mode (no MADE needed)
MADE_MCP_BACKEND=fixture \
MADE_MCP_BIN=made-mcp \
  bash scripts/mcp/made-stdio-smoke.sh

# Live mode
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 \
MADE_MCP_BIN=made-mcp \
  bash scripts/mcp/made-stdio-smoke.sh
```

The script issues one `tools/call`, asserts `"jsonrpc":"2.0"` is
present, `"isError":true` is absent, and an expected marker is
present.

## Multi-Agent vLLM E2E

`make e2e-mcp-council-vllm` proves the same real-provider council
ceremony through MCP stdio instead of direct gRPC. It builds
`made-mcp` from the checkout when `MADE_MCP_BIN` is not set, then
uses `tools/call` requests for:

- `made_register_contract`
- `made_register_agent`, once per vLLM agent
- `made_create_council`
- `made_run_council_decision`

The final response must contain multiple candidates, at least one
schema-valid candidate, a schema-valid Report winner, distinct agent
authors, and `revision_count > 0` on the winner and every candidate.

```bash
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 \
MADE_VLLM_ENDPOINT=https://vllm.example.com \
MADE_VLLM_MODEL=google/gemma-4-31B-it \
MADE_VLLM_AGENT_COUNT=3 \
  make e2e-mcp-council-vllm
```

Use the same TLS env vars as live mode when the MADE endpoint
requires server TLS or mTLS. The MADE target must be built
with `agent-vllm` and booted with `MADE_VLLM_MODEL` plus
`MADE_VLLM_ENDPOINT` so `kind=vllm` is available; the E2E also sends
per-agent `provider.endpoint`, `provider.model`, and
`provider.max_tokens` overrides through MCP.

## Manual JSON-RPC check

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | MADE_MCP_BACKEND=fixture cargo run -q -p made-mcp --locked
```

Expected:

- the server writes one JSON-RPC response per input line;
- fixture mode returns deterministic responses only when explicitly
  selected;
- live mode returns MCP tool errors instead of crashing if the gRPC
  endpoint is unreachable.

## Client configuration

The two officially-supported clients have dedicated guides:

- [Codex CLI](./mcp/codex.md) — TOML config + `codex mcp add` form.
- [Claude Desktop](./mcp/claude-desktop.md) — `claude_desktop_config.json`
  with per-OS paths.

Both share the same env-driven backend selection; the only difference
is the file location the client expects.

## Streaming caveat

`made_stream_deliberation` corresponds to `StreamDeliberation`, a
server-streaming RPC. MCP stdio is synchronous request/response, so
the adapter buffers the entire stream into a single response:

```json
{
  "task_id": "...",
  "frames": [ /* every DeliberationUpdate in order */ ],
  "winner": { /* extracted from the last result-typed frame */ }
}
```

There is no `progress`-style live emission. If your agent needs
incremental frames, call gRPC directly.
