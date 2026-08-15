# made-mcp

Hand-rolled stdio MCP (Model Context Protocol) adapter that exposes
MADE capabilities to coding agents. It can connect
to the deployable gRPC service or run the ceremony engine in process.

End-user installation, configuration snippets for Codex CLI / Claude
Desktop, and the env-var reference live in
[`docs/operations/mcp-stdio.md`](../../docs/operations/mcp-stdio.md).
This README is the developer-oriented twin: it covers running the
adapter from a checkout, the test surface, and the design choices
worth knowing when you touch the code.

## Install (registry)

```bash
cargo install made-mcp --locked
```

This pulls `made-mcp` + the vendored `made-mcp-proto` from
crates.io. The dev fallback against this repo's source tree is
`MADE_MCP_INSTALL_MODE=git bash scripts/mcp/install-made-mcp.sh`.

## Run from a checkout

```bash
# fixture mode — no MADE needed
MADE_MCP_BACKEND=fixture cargo run -p made-mcp --locked

# embedded ceremony mode — real engine, no external service, durable state
MADE_MCP_BACKEND=embedded \
MADE_MCP_REDB_PATH="${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.redb" \
  cargo run -p made-mcp --no-default-features --features embedded --locked

# live mode against a local MADE
MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055 \
  cargo run -p made-mcp --locked

# live mode over mTLS
MADE_MCP_GRPC_ENDPOINT=https://made.example.com \
MADE_MCP_GRPC_TLS_MODE=mutual \
MADE_MCP_GRPC_TLS_CA_PATH=/var/run/made-tls/ca.crt \
MADE_MCP_GRPC_TLS_CERT_PATH=/var/run/made-tls/tls.crt \
MADE_MCP_GRPC_TLS_KEY_PATH=/var/run/made-tls/tls.key \
  cargo run -p made-mcp --locked
```

The binary reads one JSON-RPC line at a time from stdin and writes
one response per non-notification message to stdout. Stderr is
structured JSON tracing (level controlled by `RUST_LOG`,
default `made_mcp=info`).

## Manual JSON-RPC walkthrough

```bash
printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | MADE_MCP_BACKEND=fixture cargo run -q -p made-mcp --locked
```

The `initialize` reply carries `serverInfo` and adapter-side metadata
(`backend`, `grpc_tls`) so the client can record what it negotiated
without an extra round trip.

For self-description after initialization, call
`made_discover_capabilities`. It projects server identity, active backend,
capability groups, artifact generators and executable tools from the same
backend-filtered catalog used by `tools/list`. `made_get_help` accepts
`audience: user` or `audience: agent`; the latter includes preconditions,
authority boundaries, delegated-host sequencing and explicit error handling.

## Tool dispatch table

| MCP tool                          | gRPC RPC                          | Notes |
|-----------------------------------|-----------------------------------|-------|
| `made_deliberate`               | `Deliberate`                      | sync |
| `made_stream_deliberation`      | `StreamDeliberation`              | buffered: stream collected into one response |
| `made_get_deliberation_result`  | `GetDeliberationResult`           | sync |
| `made_orchestrate`              | `Orchestrate`                     | sync |
| `made_create_council`           | `CreateCouncil`                   | control plane |
| `made_list_councils`            | `ListCouncils`                    | read |
| `made_delete_council`           | `DeleteCouncil`                   | idempotent control plane |
| `made_register_agent`           | `RegisterAgent`                   | control plane |
| `made_unregister_agent`         | `UnregisterAgent`                 | control plane |
| `made_process_trigger_event`    | `ProcessTriggerEvent`             | event ingest |
| `made_run_council_decision`     | `RunCouncilDecision`              | sync; structured-output decision |
| `made_register_contract`        | `RegisterContract`                | control plane |
| `made_list_contracts`           | `ListContracts`                   | read |
| `made_delete_contract`          | `DeleteContract`                  | idempotent control plane |
| `made_run_ceremony`             | `RunCeremony`                     | sync; runs a YAML ceremony to a terminal state |
| `made_get_ceremony_instance`    | `GetCeremonyInstance`             | inspect a persistent instance |
| `made_list_ceremony_instances`  | `ListCeremonyInstances`           | discover persistent instances |
| `made_start_ceremony`           | `StartCeremony`                   | start supplied YAML without advancing |
| `made_start_published_ceremony` | `StartPublishedCeremony`          | start an immutable published definition |
| `made_run_ceremony_step`        | `RunCeremonyStep`                 | invoke the configured server-owned step handler |
| `made_apply_ceremony_transition` | `ApplyCeremonyTransition`         | apply an enabled transition |
| `made_approve_ceremony_guard`   | `ApproveCeremonyGuard`            | record an explicit human guard approval |
| `made_defer_ceremony_guard`     | `DeferCeremonyGuard`              | preserve a human deferral |
| `made_request_ceremony_intervention` | `RequestCeremonyIntervention` | open a participant request |
| `made_respond_to_ceremony_intervention` | `RespondToCeremonyIntervention` | record a targeted response |
| `made_close_ceremony_intervention` | `CloseCeremonyIntervention`    | close a participant request |
| `made_collect_ceremony_evidence` | `CollectCeremonyEvidence`        | attach evidence from a configured source |
| `made_assert_ceremony_reason`   | `AssertCeremonyReason`            | record a participant-attributed reason |
| `made_validate_ceremony_draft`  | `ValidateCeremonyDraft`           | validate without publishing |
| `made_explain_ceremony_draft`   | `ExplainCeremonyDraft`            | explain structure and findings |
| `made_publish_ceremony_definition` | `PublishCeremonyDefinition`    | publish an immutable definition |
| `made_diff_ceremony_definitions` | `DiffCeremonyDefinitions`        | compare two definitions |
| `made_bind_ceremony_participants` | `BindCeremonyParticipants`      | seat participants in declared roles |
| `made_get_status`               | `GetStatus`                       | observability |
| `made_get_metrics`              | `GetMetrics`                      | observability |

These 35 backend-owned tools map 1:1 to the 35 RPCs in the MADE gRPC
service. Every server composition additionally advertises the two server-owned
discovery/help tools described below.

The embedded backend also exposes four tools that intentionally have no gRPC
mapping:

| MCP tool | Purpose |
|----------|---------|
| `made_design_ceremony` | Turn structured intent into an analysed, unpublished linear ceremony draft. |
| `made_claim_ceremony_step` | Lease the next step for real work performed by the MCP host; claiming performs no work. |
| `made_complete_ceremony_step` | Record the observable status, structured output, and evidence of a previously claimed host-executed step. |
| `made_generate_ceremony_report` | Render one or more persisted instances and their audit journals as deterministic Markdown. |

The incremental ceremony controls allow the host to pause between actions.
Human guard approval is never inferred by the server; the client must obtain
the person's decision
before it invokes the approval tool. Dynamic interventions likewise coordinate
the live agenda without bypassing host permissions or ceremony guards. Omitting
`target_role_ids` addresses the whole table; supplying it scopes the request to
those roles. Responses and interventions retain insertion order in the instance.
Server-owned execution through `made_run_ceremony_step` is valid only when
the embedding host configured a real step handler. The bundled default may use
`NoopCeremonyStepHandler`, whose empty completed result proves wiring rather
than operational work. For delegated-host execution, claim the exact next
step, perform the real work through authorized host capabilities, complete it
with observable output/evidence, refresh the instance, and only then apply an
enabled transition. These adapters invoke existing application use cases and
add no external authority or approval policy.
The default embedded composition stores state in memory. Process-restart
recovery requires the host to supply durable repositories for ceremony
instances, mounted definitions, and transcript context.

`made_generate_ceremony_report` accepts a non-empty, duplicate-free
`ceremony_ids` array and an optional presentation title. Unknown ids fail the
whole request. Its structured result contains the Markdown, selected ids,
completed/incomplete counts, definition versions and available digests, plus
`persisted: false`: it never writes a report file. Sections and ordering are
stable, untrusted values are JSON-encoded inside safe variable-length fences,
and persisted outputs/evidence are not truncated. Split large selections into
smaller calls when the MCP client has a response-size limit.

Every server composition also exposes these adapter-owned tools:

| MCP tool | Purpose |
|----------|---------|
| `made_discover_capabilities` | Return version, backend, capability groups, tools, and generators from the active catalog. |
| `made_get_help` | Return structured plus Markdown guidance for a `user` or an `agent`. |

Discovery marks the report tool in two machine-readable places:
`tools[].report_generator` and `artifact_generators[]`. The generator record
also identifies `structuredContent.report_markdown` and states that the host,
not the tool, owns persistence. Help workflows are filtered against the active
catalog, and coverage tests reject a help response that references a tool the
same backend does not advertise.

Mappings live in `src/grpc/{json_to_proto.rs,proto_to_json.rs}` —
**hand-written field-by-field**. A new proto field is a one-PR
change: add the schema key in `protocol.rs`, add the request mapper
in `json_to_proto.rs`, add the response mapper in `proto_to_json.rs`.

> `made-mcp` builds against `made-mcp-proto`, a **vendored copy** of
> `underpass.made.v1` kept byte-identical to `crates/made-proto/proto`
> so this crate can publish independently. The two `.proto` files must be
> kept in sync by hand — there is no automated cross-copy diff. (The
> `tools_catalog_is_derived_one_for_one_from_grpc_service` test only
> enforces the tool↔RPC 1:1 mapping against this crate's own vendored
> copy.)

## Design choices

1. **No MCP SDK.** Tokio + serde_json + tonic + a handful of small
   helpers. The wire protocol stays in lock-step with the proto
   contract because the team owns every byte.

2. **Backend trait as the single seam.** `MadeMcpToolBackend` has
   live gRPC, embedded ceremony, and deterministic fixture adapters.
   Each backend filters `tools/list` to operations it can honor.
   Selection is env-driven and fail-fast — there is no silent fallback
   when the requested backend is misconfigured or not compiled.

3. **JSON-RPC stays sync.** MCP stdio is request/response; the
   adapter does not implement server progress notifications.
   `StreamDeliberation` buffers the full server stream into a
   single response with a `frames` array and a `winner` field
   extracted from the last `result`-typed frame.

4. **Field-for-field mapping.** No `serde_json::to_value(proto)`
   shortcuts. Enums collapse to stable string labels. A new proto
   field that lands without an MCP mapper update is a review-time
   miss, not a silent drop.

5. **Error result shape.** Tool errors come back as `isError: true`
   inside the tool result, per MCP spec — **not** as JSON-RPC
   errors. JSON-RPC `error` codes are reserved for protocol-level
   issues (parse error, missing params, unsupported method).

6. **Privacy-safe telemetry.** Tool error messages are
   SHA-256-prefix hashed before they go into metrics. The full
   message reaches the caller through the tool result text (where
   the agent wanted it) and the structured trace event (where the
   operator opted into debug logging).

## Tests

```bash
cargo test -p made-mcp --locked
```

- `src/protocol.rs::tests` — initialize / tools/list shape, every
  tool definition is present, success/error envelopes.
- `src/server.rs::tests` — JSON-RPC dispatch paths and error codes.
- `src/backend.rs::tests` — TLS mode parsing + URL upgrade.
- `src/fixture.rs::tests` — every tool has a canned fixture and the
  fixture envelope matches the live response shape.
- `src/observability.rs::tests` — error-kind labels and the recursive
  size approximator used in trace events.

### Real-kernel container integration test

A separate `tests/real_kernel.rs` boots the published
`ghcr.io/underpass-ai/made:latest` image via
testcontainers, spawns this crate's binary against its mapped gRPC
port, and exercises `initialize`, verifies `tools/list` against machine-readable
discovery (currently 35 gRPC-backed plus two server-owned tools), and calls the
four simplest read-only RPCs.
The test is gated by the `container-tests` Cargo feature so the
default workspace `cargo test --workspace` stays fast + network-free.

```bash
cargo test -p made-mcp --features container-tests
```

The default `cargo test --workspace` does NOT compile testcontainers
or pull the image.

## Common pitfalls

- **Stdout pollution.** Anything written to stdout that is not a
  JSON-RPC response will desync the client. Use `tracing` (which
  writes to stderr); avoid `println!`.
- **Blocking the loop.** The dispatcher awaits each tool call
  serially. A long-running call blocks subsequent inputs — by
  design, since most agents wait on the previous response before
  emitting the next request.
- **Env var typos.** TLS auto-detection is permissive: setting
  `MADE_MCP_GRPC_TLS_CERT_PATH` alone (no key) is silently
  ignored, falling back to `server`. The startup log emits the
  active `grpc_tls` mode — check it when debugging.
