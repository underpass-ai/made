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

Both responses are derived against the same catalog filter as `tools/list`.
An embedded build therefore advertises its report generator, while a backend
that cannot execute that tool neither lists it nor recommends its workflow.

The 35 backend-owned MCP tools are 1:1 with MADE's 35 gRPC RPCs.
Together with the two server-owned discovery/help tools above, gRPC mode
advertises 37 executable tools:

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
| `made_run_ceremony`             | `RunCeremony`                         | Execute a declarative ceremony YAML; returns final state, per-step winning contributions, and the Mermaid conversation diagram. |
| `made_register_contract`        | `RegisterContract`                    | Register an `OutputContract` in the contract registry. |
| `made_list_contracts`           | `ListContracts`                       | Enumerate registered contracts. |
| `made_delete_contract`          | `DeleteContract`                      | Idempotent contract delete. |
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
| `made_get_status`               | `GetStatus`                           | Service health, version, uptime, optional stats. |
| `made_get_metrics`              | `GetMetrics`                          | Statistics snapshot. |

The MADE API is **respected at 100%** — every proto field has
an explicit JSON key in both the tool input schema and the response.
No flattening, no silent drops. Enums (e.g. `DeliberationPhase`) map
to stable string labels (`DELIBERATION_PHASE_PROPOSING`, …).

### Durable ceremony reports (embedded)

`made_generate_ceremony_report` is an embedded-only, read-only extension. A
call supplies `ceremony_ids` as a non-empty array with no duplicates and may
supply `title`. Unknown ids fail the whole call; caller order is preserved.

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
  redb state file named by `MADE_MCP_REDB_PATH`. That variable is mandatory:
  where ceremony state lives is an operator decision, and the binary exits
  with code 2 rather than inventing a location or quietly running on memory
  that dies with the process. The isolated
  build exposes one-shot execution plus persistent incremental controls for
  starting, inspecting, stepping, claiming/completing host-owned work,
  explicitly approving a human guard, and applying a transition. It can also
  generate deterministic Markdown reports from one or more persisted ceremony
  snapshots and audit journals.
  Participants can open, answer, and close dynamic opinion, investigation, or
  action requests while the ceremony remains active. It requires no
  MADE service, gRPC, protobuf, NATS, or database.
- **`fixture`** — returns canned responses for every tool. Useful for
  client wiring, demos, and tool-choice validation **without** a
  running MADE.

```bash
MADE_MCP_BACKEND=fixture cargo run -p made-mcp --locked

MADE_MCP_BACKEND=embedded \
MADE_MCP_REDB_PATH="${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.redb" \
  cargo run -p made-mcp --locked
```

### Sharing one ceremony store between two agent hosts

The default engine takes one process at a time, and MADE's store is one file
at one path rather than one per project, so two agent hosts — Claude Code and
Codex CLI both running the plugin — collide unconditionally. Whichever starts
first owns the store; the other reports `CONNECTION_CLOSED` and its ceremony
engine never starts.

The `sqlite` engine removes that. It is WAL-mode SQLite: readers never block
the writer, and a second writer waits for the commit lock instead of being
refused. Opt-in, because it brings a C toolchain into the build.

```bash
# a binary that carries the engine
cargo install made-mcp --features sqlite

# starting fresh: ask for sqlite when the store is created
MADE_MCP_ENGINE=sqlite MADE_MCP_BACKEND=embedded \
MADE_MCP_REDB_PATH=~/.local/state/underpass-made/ceremonies.sqlite3 made-mcp

# already have ceremonies: convert, then point both hosts at the result
made-mcp convert ceremonies.redb ceremonies.sqlite3 --engine sqlite
```

Through the plugin there is no path to set: `MADE_MCP_ENGINE=sqlite` makes the
launcher pick `ceremonies.sqlite3` beside the default, and a converted store
already sitting there is opened without asking. With both files present the
launcher keeps the redb default rather than choosing for you.

One thing the plugin does need to be told. A release bundle ships its own
`bin/made-mcp`, built without the sqlite engine — that is what keeps the
default install free of a C toolchain — and the launcher prefers it over
anything on `PATH`. So an operator who built the engine has to say which
binary to run:

```bash
MADE_MCP_BIN="$HOME/.cargo/bin/made-mcp" MADE_MCP_ENGINE=sqlite  # in both hosts' registrations
```

It selects the executable and nothing else; the state path, the engine and the
legacy import still apply. Installing the plugin straight from the repository
has no `bin/`, so there the `PATH` binary is used already and this is not
needed.

**A store is opened by the engine that wrote it, always.** Both formats
announce themselves in their first bytes, so there is no marker file to keep
in sync and no way to open a store with the wrong engine: `MADE_MCP_ENGINE`
decides only what a *new* store becomes, and asking for a different engine
than an existing store is refused by name.

The conversion copies rows table by table rather than replaying the audit
journal. That is not a shortcut: a ceremony store is state plus a journal, not
a log with derived projections, so replaying would rebuild the facts and lose
what they are evidence of. It reads its source only, refuses a destination
that already holds a store, and prints a receipt of what moved.

What it costs: a C dependency in the opt-in build. What it does not cost here
is a new C library — `sqlx` already brings the same one, so the engine adds
five pure-Rust crates and nothing else. A binary built without the feature
still recognises a SQLite store and refuses it by name rather than failing
obscurely.

redb takes an exclusive lock on that file: one MCP process owns a given
state file at a time. What survives a restart is bounded by the
[published-definition boundary](./capability-verification.md) — an instance
started from a published definition rehydrates, one started from supplied
YAML keeps its snapshot but cannot reload its definition, and
`made_list_ceremony_instances` reports the latter as
`"rehydratable": false` instead of failing the whole listing. The full
loop is in the
[embedded ceremony execution runbook](./embedded-ceremony-execution.md).

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

The embedded-only intervention tools are:

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
| `MADE_MCP_REDB_PATH`           | state file the embedded backend opens. Required when `BACKEND=embedded`. |
| `MADE_MCP_ENGINE`              | engine for a **new** store: `redb` (default) or `sqlite`. An existing store always opens on whatever wrote it. |
| `MADE_MCP_BIN`                 | plugin launchers only: the executable to run, overriding the bundled binary. Needed to reach a `--features sqlite` build from a release bundle. |
| `MADE_MCP_LEGACY_REDB_PATH`    | Optional read-only Choreographer source imported once into a new `MADE_MCP_REDB_PATH`. |
| `MADE_MCP_GRPC_ENDPOINT`       | URL the MCP connects to. Required when `BACKEND=grpc`.                   |
| `MADE_MCP_GRPC_TLS_MODE`       | `disabled` / `server` / `mutual`. Auto-derived when omitted.             |
| `MADE_MCP_GRPC_TLS_CA_PATH`    | PEM CA bundle. Implies `server` mode when set.                           |
| `MADE_MCP_GRPC_TLS_CERT_PATH`  | Client cert PEM (mutual). Implies `mutual` mode when set.                |
| `MADE_MCP_GRPC_TLS_KEY_PATH`   | Client key PEM (mutual). Implies `mutual` mode when set.                 |
| `MADE_MCP_GRPC_TLS_DOMAIN_NAME`| TLS SNI/domain override when cert CN/SAN differs from the URL host.      |

`RUST_LOG=made_mcp=debug` enables structured per-tool-call tracing
on stderr (stdout is reserved for JSON-RPC).
Legacy migration events are visible at the default log level through the
`made_adapters::redb=info` target. They report the migration id, source open
mode, source SHA-256 and bounded row counts, never ceremony payloads.

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
