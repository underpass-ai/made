# Choreographer Backlog

Snapshot date: 2026-04-25; honest re-audit 2026-05-11; PIR framing
dropped 2026-05-12 (PIR is owned by a separate project — this backlog
tracks Choreographer's own stack-readiness, not any one downstream
consumer).

Companion documents:

- [`stack-gap-analysis.md`](./stack-gap-analysis.md)
- [`operations/mcp-stdio.md`](./operations/mcp-stdio.md) — installable
  stdio MCP adapter UX.

The goal is to keep Choreographer trustworthy as a stack peer:
real execution, real context, structured council outputs, causal
metadata, provider-backed councils, honest transport, TLS, an
agent-facing surface (gRPC + MCP), and reproducible stack E2E.

## Executive summary

As of 2026-05-11 the eight stack-readiness areas resolve as follows:

| # | Area | State |
|---|---|---|
| 1 | real Runtime execution | done (adapter + env-driven wiring) |
| 2 | typed external context input | done (typed `ExternalContextBundle` flowing trigger -> task -> deliberation) |
| 3 | structured, contract-validated council outputs | done (structured-output mode + deterministic `NoValidProposal` failure) |
| 4 | complete causal metadata propagation | done (Epic 5) |
| 5 | provider-backed council materialization | done (`DispatchingAgentFactory` wired with `noop`/`anthropic`/`openai`/`vllm` arms) |
| 6 | honest and durable transport semantics | done (AsyncAPI now declares plain core NATS; JetStream deferred) |
| 7 | real TLS / mTLS posture | mostly done (server-side TLS wired with `none`/`server`/`mutual`; chart honest; outbound client TLS + handshake-level integration test deferred) |
| 8 | stack-level end-to-end proofs | partial (E2E covers Noop council + causal metadata; real-council + runtime legs missing) |

Two surfaces beyond the eight areas:

- **MCP stdio adapter** — `crates/choreo-mcp` ships a hand-rolled
  stdio MCP server that exposes every RPC of `underpass.choreo.v1`
  as a `choreo_*` tool. End-user docs live at
  [`docs/operations/mcp-stdio.md`](./operations/mcp-stdio.md); per-
  client snippets for Codex CLI and Claude Desktop live under
  `docs/operations/mcp/`. Foundation merged 2026-05-12; the
  distribution slice ships install + smoke scripts and a top-README
  link.
- **Downstream product integrations (PIR, payments incident response,
  custom agentic flows)** are **out of scope for this repo**. The
  product owns its own deliberation surface; Choreographer's job is to
  expose a clean, fully-typed gRPC API plus the MCP wrapping so any
  agentic consumer can drive it.

Genuinely open work: the outbound-TLS leg of Epic 8, the crates.io
distribution debt for `choreo-mcp` (needs the proto tree vendored
into a separate crate before `cargo install` from a registry will
work), Epic 9 (a dedicated council-decision RPC if and when a
consumer asks for more than the generic `Deliberate` / `Orchestrate`),
Epic 10 (report artifact), and the missing real-council / runtime
legs of Epic 11.

The recommended remaining execution order is:

- Phase 3: outbound client TLS + `choreo-mcp` crates.io distribution
- Phase 4: dedicated agent-facing RPC + report artifact (only if a
  consumer asks for them)
- Phase 5: stack E2E with a real council and the Runtime executor

## Out of scope

This backlog does not include:

- moving kernel graph semantics into Choreographer
- making Choreographer domain-specific (payments, incidents, …)
- implementing any downstream product (PIR, payments incident
  response, etc.) in this repository

## Priorities

### P0 — hard blockers (remaining)

These items still block downstream consumer integration.

- Runtime gRPC client TLS in `RuntimeExecutor::connect` (Epic 8 leg)
- dedicated consumer-facing RPC surface (Epic 9)
- structured report artifact support (Epic 10)
- stack E2E proof with real council + runtime executor (Epic 11 leg)

Already cleared: Runtime executor adapter (Epic 1), Kernel context
boundary (Epic 2), structured council output contracts (Epic 3),
causal metadata model (Epic 5), provider-backed agent factory
composition (Epic 6), honest transport semantics (Epic 7 — declared
plain NATS), gRPC server TLS/mTLS posture (Epic 8 server side).

### P1 — required before production

- contract-aware validators — JSON Schema and bounded-event-shape
  variants still missing (Epic 4 has the format-level slice)
- release-gate stack smoke
- the basic four scenarios of the e2e-runner already exist; release-gate
  hooks need to wire them in for cut tags

### P2 — useful after first integration

- bus-native downstream coupling
- per-proposal streaming for expert councils
- richer score explainability

## Phase 1 — Runtime And Context Foundations

### Epic 1. Runtime executor adapter

Status: done

Current state:

- `RuntimeExecutor` adapter implements `ExecutorPort` against the
  Underpass Runtime gRPC; creates ephemeral sessions, invokes tools,
  closes sessions, maps `Succeeded`/`Failed`/`Denied`/transport errors
  distinctly
- `ExecutorBackendConfig` selected from `CHOREO_EXECUTOR_KIND=noop|runtime`
  plus principal env vars; `NoopExecutor` stays as explicit fallback
- adapter unit tests cover success, denial, transport error, env
  loading, and option-vs-attributes precedence; compose-level test
  wires the runtime adapter against a stub gRPC server

Relevant code:

- [`crates/choreo-adapters/src/runtime.rs`](../crates/choreo-adapters/src/runtime.rs)
- [`crates/choreo/src/compose.rs`](../crates/choreo/src/compose.rs) (`wire_executor`)
- [`crates/choreo-core/src/ports/executor.rs`](../crates/choreo-core/src/ports/executor.rs)

Progress as of 2026-05-11: implementation landed in commit `fab9bfb`
(PR #43). All four acceptance criteria + the three required tests
listed below are present in the repo today.

#### Deliverables

1. add a Runtime gRPC executor adapter in `choreo-adapters`
2. map winner proposals plus execution options into runtime session creation
3. support runtime session metadata for:
   - external incident id
   - external incident run id
   - specialist / council identity
   - tool / governance / success profile
4. map runtime invocation outcomes into `ExecutionOutcome`
5. classify:
   - transport errors
   - governed denials
   - runtime failures
   - runtime success

#### Acceptance criteria

- `compose()` can wire Runtime executor behind configuration
- `NoopExecutor` remains available as explicit fallback, not as the only path
- orchestration against a fake or bufconn runtime returns real execution ids
- denial and failure semantics are distinguishable in tests

#### Tests required

- unit tests for request mapping
- adapter integration test against stub gRPC server
- use-case integration test proving `OrchestrateUseCase` emits correct events
  for success and failure with the runtime adapter wired

### Epic 2. Kernel context boundary

Status: done (option A — caller-materialized context)

Current state:

- typed `ExternalContextBundle` (with `ContextSummary`, `ContextItem`,
  `ContextReference`, bounded sizes, and serde + roundtrip tests)
  lives in the core
- `Task` carries `Option<ExternalContextBundle>` through
  `new_with_context` / `new_with_metadata`
- proto exposes the bundle on `Task` and on `TriggerEvent`; gRPC mappers
  consume it
- `AutoDispatchService` propagates the trigger's bundle into the task;
  `DeliberateUseCase` threads it into `DraftRequest.external_context`
  with a covering test

Relevant code:

- [`crates/choreo-core/src/entities/external_context.rs`](../crates/choreo-core/src/entities/external_context.rs)
- [`crates/choreo-core/src/entities/task.rs`](../crates/choreo-core/src/entities/task.rs)
- [`crates/choreo-adapters/src/grpc/mappers/task.rs`](../crates/choreo-adapters/src/grpc/mappers/task.rs)
- [`crates/choreo-adapters/src/grpc/mappers/event.rs`](../crates/choreo-adapters/src/grpc/mappers/event.rs)
- [`crates/choreo-app/src/services/auto_dispatch.rs`](../crates/choreo-app/src/services/auto_dispatch.rs)
- [`crates/choreo-app/src/usecases/deliberate.rs`](../crates/choreo-app/src/usecases/deliberate.rs)

Progress as of 2026-05-11: implementation landed in commit `fab9bfb`
(PR #43). Option B (a Kernel adapter port owned by Choreographer)
remains explicitly deferred — the backlog recommended option A.

#### Deliverables

1. define one explicit context ingestion boundary:
   - option A: caller fetches kernel context and passes it to Choreographer
   - option B: Choreographer can fetch context itself through a new port
2. choose one as the production path for the first downstream integration
3. define a stable structured bundle shape for expert councils:
   - incident summary
   - prior findings
   - prior decisions
   - evidence references
   - failed remediations
4. make that bundle addressable and testable

#### Recommendation

For the first slice, prefer caller-materialized context:

- the consumer remains the kernel-first owner
- Choreographer remains domain-agnostic
- the integration boundary is cleaner

That means Choreographer must still gain a first-class notion of
"structured external context bundle", but it does not have to own
Kernel transport in v1.

#### Acceptance criteria

- one typed council input can carry a bounded incident context bundle
- the council path can consume that bundle without lossy string stuffing
- the chosen bundle shape has contract tests

#### Tests required

- serialization tests for the bundle shape
- one end-to-end deliberation test with a realistic external context bundle

## Phase 2 — Contract-Shaped Deliberation

### Epic 3. Structured council outputs

Status: done

Current state:

- `OutputContract` value object with `OutputFieldRule` and
  `OutputFormat::JsonObject`; serde + validation tests in place
- `TaskConstraints::with_output_contract` carries the contract; the
  proto contract surfaces it inside `Constraints`
- `DeliberateUseCase` switches into structured-output mode when a
  contract is set; valid proposals are reprioritized before any winner
  selection so invalid outputs cannot leak as winners
- deterministic failure: `DomainError::NoValidProposal { contract_id }`;
  `OrchestrateUseCase` maps it to `TaskFailed` with
  `error_kind = "deliberation.no_valid_proposal"`
- regression tests prove invalid proposals lose to valid ones even at
  higher score, and that an all-invalid run fails deterministically

Relevant code:

- [`crates/choreo-core/src/value_objects/output_contract.rs`](../crates/choreo-core/src/value_objects/output_contract.rs)
- [`crates/choreo-core/src/entities/proposal.rs`](../crates/choreo-core/src/entities/proposal.rs)
- [`crates/choreo-app/src/usecases/deliberate.rs`](../crates/choreo-app/src/usecases/deliberate.rs) (`prioritize_valid_outputs`, `pick_winner`)

Progress as of 2026-05-11: implementation landed in commit `fab9bfb`
(PR #43).

#### Deliverables

1. define a structured-output mode for councils
2. allow a council invocation to declare:
   - output contract id
   - JSON schema or equivalent validator
   - allowed decision set
3. ensure winner selection happens only among valid proposals
4. define deterministic failure semantics when:
   - every proposal is invalid
   - schema validation fails
   - allowed decision validation fails

#### Acceptance criteria

- a council can be run in "contract output" mode
- invalid outputs do not leak out as winners
- caller can distinguish:
   - no valid proposal
   - transport failure
   - provider failure
   - validation failure

#### Tests required

- schema validator tests
- allowed-decision validator tests
- deliberation test where one invalid proposal loses to a valid one
- deliberation test where all proposals are invalid and the run fails
  deterministically

### Epic 4. Contract-aware validators

Status: mostly done

Current state:

- four validators wired through `Vec<Arc<dyn ValidatorPort>>` in
  `compose.rs`: `ContentNonEmptyValidator`, `JsonObjectOutputValidator`,
  `RequiredFieldsValidator`, `AllowedStringValuesValidator`
- `JsonObjectOutputValidator` enforces JSON-object root for
  `OutputFormat::JsonObject`
- `RequiredFieldsValidator` enforces required fields from
  `OutputContract.fields`
- `AllowedStringValuesValidator` enforces enum / allowed-decision
  membership
- unit tests cover happy path, missing fields, unknown allowed values,
  and no-op behaviour when no contract is set
- validator reports stay domain-agnostic (only `kind`/`passed`/`summary`/`Attributes`)

Relevant code:

- [`crates/choreo-adapters/src/validators.rs`](../crates/choreo-adapters/src/validators.rs)
- [`crates/choreo/src/compose.rs`](../crates/choreo/src/compose.rs)

Progress as of 2026-05-11: format-level slice landed in `fab9bfb`.

#### Deliverables (remaining)

Still to add:

- general JSON Schema validator (Cargo manifest has no `jsonschema`
  crate today; current implementation only checks JSON-object format)
- bounded event proposal shape validator
- report artifact shape validator (depends on Epic 10)

#### Acceptance criteria

- validators are composable through the existing validation pipeline (done)
- validator reports remain domain-agnostic from Choreographer's perspective (done)
- downstream council contracts can be enforced with no handwritten
  post-processing hacks (depends on the JSON Schema + report-shape
  validators above)

### Epic 5. Task / council metadata model

Status: done for the domain-agnostic core slice

Current state:

- `Task` now has integration-neutral `TaskMetadata`
- `EventEnvelope` carries both `correlation_id` and `causation_id`
- inbound trigger metadata is converted into task metadata
- lifecycle events preserve causal metadata through deliberation and orchestration

Progress as of 2026-04-26:

- added first-class `TaskMetadata`
- added `source_event_id`, `causation_id`, and `correlation_id` propagation
- added proto and gRPC mapper support for task metadata
- kept application-owned identifiers out of the core; product/domain ids remain
  in `Task.attributes` or `ExternalContextBundle.metadata`
- added tests proving causal metadata reaches deliberation, dispatch, completion,
  and failure events
- wired `execution_profile` into executor options, with explicit call options
  taking precedence

Relevant code:

- [`crates/choreo-core/src/entities/task.rs`](../crates/choreo-core/src/entities/task.rs)
- [`crates/choreo-core/src/events/envelope.rs`](../crates/choreo-core/src/events/envelope.rs)

#### Deliverables

Introduce a first-class metadata surface that can carry:

- source event id
- causation id
- correlation id
- council contract id
- output contract id
- execution profile metadata

Application-specific identifiers must remain outside the core metadata model.
Use `Task.attributes` or `ExternalContextBundle.metadata` for product/domain
ids such as incidents, cases, claims, shipments, studies, or similar concepts.

#### Acceptance criteria

- metadata survives trigger -> task -> deliberation -> orchestration -> outbound event
- metadata can be inspected in tests without parsing arbitrary blobs
- execution-profile metadata is wired into the executor path where applicable

## Phase 3 — Provider And Composition Readiness

### Epic 6. Provider-backed agent factory composition

Status: done

Current state:

- `DispatchingAgentFactory` (in `crates/choreo-adapters/src/agents/factory.rs`)
  implements `AgentFactoryPort` and dispatches on `descriptor.kind`:
  - `"noop"` — always available
  - `"anthropic"` — gated on `agent-anthropic` feature + `CHOREO_ANTHROPIC_API_KEY`
  - `"openai"` — gated on `agent-openai` feature + `CHOREO_OPENAI_API_KEY`
  - `"vllm"` — gated on `agent-vllm` feature + `CHOREO_VLLM_MODEL` + `CHOREO_VLLM_ENDPOINT`
- per-descriptor overrides: `provider.model`, `provider.endpoint`,
  `provider.max_tokens` on the descriptor's `attributes`
- credentials live ONLY in env (descriptors are persisted in Postgres,
  so secrets must not flow through them)
- the binary wires `DispatchingAgentFactory::from_env()` unconditionally;
  startup log emits `agent_kinds=...` listing the supported set
- `supported_kinds()` accessor returns the live list so operators can
  see which kinds the deployment will accept on `RegisterAgent`

Relevant code:

- [`crates/choreo-adapters/src/agents/factory.rs`](../crates/choreo-adapters/src/agents/factory.rs)
- [`crates/choreo/src/compose.rs`](../crates/choreo/src/compose.rs)
- [`crates/choreo-adapters/src/agents/`](../crates/choreo-adapters/src/agents/)

Progress as of 2026-05-11: implementation landed in the next PR.
`NoopAgentFactory` remains available as a single-kind factory for
tests; the production binary uses `DispatchingAgentFactory` only.

#### Deliverables

1. add a dispatching `AgentFactoryPort` composition root
2. support configured real `kind` values in the binary
3. make expert councils materializable from descriptors
4. ensure persistence + rehydration of real agent descriptors works

#### Acceptance criteria

- `RegisterAgent` can materialize at least one real provider kind
- persisted descriptors can be resolved back into live agents
- the binary documents which provider kinds are truly supported

#### Tests required

- composition tests for provider dispatch
- persistence rehydration tests for provider-backed descriptors

## Phase 4 — Transport And Security Honesty

### Epic 7. Honest broker semantics

Status: done (option 1 — declared plain NATS)

Current state:

- `NatsMessaging` uses `Client::publish_with_headers` (core NATS);
  `NatsTriggerSubscriber` uses `client.subscribe` (core NATS,
  fire-and-forget). No JetStream stream / durable consumer / ack /
  replay policy is used by the adapter.
- AsyncAPI now declares the broker as **plain core NATS pub/sub**
  with the matching disclaimer in the `servers.nats.description`
  field; the implementation–spec gap from the original audit is closed.
- `docs/stack-gap-analysis.md` §4 retitled "Broker semantics declared
  honestly as plain NATS".
- The docker-compose / kubernetes test fixtures may still start the
  NATS server with `-js`; this is harmless (server-side JetStream
  capability is independent of whether the client opens it) and
  preserves an upgrade path if option 2 is chosen later.

Decision rationale: the expected first downstream integration uses direct
gRPC (Epic 9), not the bus. Plain NATS is sufficient. Implementing
real JetStream semantics (stream + durable consumer + ack + replay)
is deferred to a future epic gated on actual bus-coupling demand.

Relevant code:

- [`crates/choreo-adapters/src/nats/messaging.rs`](../crates/choreo-adapters/src/nats/messaging.rs)
- [`crates/choreo-adapters/src/nats/subscriber.rs`](../crates/choreo-adapters/src/nats/subscriber.rs)
- [`specs/asyncapi/choreographer.asyncapi.yaml`](../specs/asyncapi/choreographer.asyncapi.yaml)

#### Deliverables

Choose one:

1. explicitly declare Choreographer as plain NATS
2. or implement true JetStream semantics:
   - stream
   - durable consumer
   - ack
   - replay / delivery policy

#### Recommendation

For a critical downstream integration, prefer real JetStream semantics if bus coupling
is expected later. If not, document plain NATS honestly and keep the first downstream
integration on direct gRPC.

#### Acceptance criteria

- code, docs, tests, and spec all say the same thing
- transport semantics are exercised in integration tests

### Epic 8. TLS / mTLS parity

Status: mostly done (gRPC server side); Runtime client TLS deferred.

Current state (2026-05-11):

- gRPC server in `crates/choreo/src/runtime.rs` builds with
  `ServerTlsConfig::new().identity(...)` (server mode) or additionally
  `client_ca_root(...)` (mutual mode), driven by the new
  `GrpcTlsConfig` enum in `ServiceConfig`. PEM files are read at
  startup; a misconfigured deployment fails fast.
- `EnvConfiguration` reads `CHOREO_GRPC_TLS_MODE` (`none`/`server`/`mutual`),
  `CHOREO_GRPC_TLS_CERT_PATH`, `CHOREO_GRPC_TLS_KEY_PATH`, and (for mutual)
  `CHOREO_GRPC_TLS_CLIENT_CA_PATH`. Validation surfaces missing-path
  combinations as `DomainError::EmptyField` and an invalid mode as
  `InvariantViolated`.
- Chart template (`charts/choreographer/templates/deployment.yaml`) mounts
  `tls.existingSecret` read-only at `/etc/choreographer/tls` and passes
  the matching env vars; rendering with `tls.mode != "none"` but no
  `existingSecret` fails the helm template with an explicit message.
- `scripts/ci/helm-lint.sh` gate 4 asserts the rendered manifest for
  both `server` and `mutual` modes carries the expected env vars and
  volume mount, and that `server` mode does NOT carry the client-CA
  env var.
- `values.yaml` `tls.mode` and `tls.existingSecret` are now honest
  configuration with documented secret layout.

Remaining work (deferred to a follow-up epic slice):

- Runtime gRPC client TLS in `RuntimeExecutor::connect` — still uses
  plain `Endpoint::from_shared(...).connect()`. Out-of-process mTLS
  to the Runtime service requires a coordinated identity rollout with
  the runtime team.
- Rust integration test that performs an actual TLS handshake against
  the choreographer (e.g. with `rcgen` to generate a self-signed cert
  in the test). Today the wiring is exercised by helm-lint gate 4
  plus the env-loading unit tests; the handshake itself is implicitly
  validated by the tonic library's invariants but not asserted
  end-to-end from this repo.

Relevant code:

- [`crates/choreo/src/runtime.rs`](../crates/choreo/src/runtime.rs)
- [`crates/choreo-adapters/src/config.rs`](../crates/choreo-adapters/src/config.rs)
- [`crates/choreo-core/src/ports/configuration.rs`](../crates/choreo-core/src/ports/configuration.rs)
- [`charts/choreographer/templates/deployment.yaml`](../charts/choreographer/templates/deployment.yaml)
- [`scripts/ci/helm-lint.sh`](../scripts/ci/helm-lint.sh)
- [`crates/choreo-adapters/src/runtime.rs`](../crates/choreo-adapters/src/runtime.rs) (Runtime client — still no TLS)

#### Deliverables

1. add server TLS/mTLS wiring if the chart surface is kept
2. or remove unsupported chart keys until real
3. ensure client-side TLS exists for Runtime calls

#### Acceptance criteria

- every declared TLS value has a code path behind it
- there is at least one integration test for enabled TLS mode

## Phase 5 — Consumer-Facing Integration Surface

### Epic 9. Specialist-grade RPC surface

Status: not started

Current state:

- proto exposes `Deliberate`, `StreamDeliberation`,
  `GetDeliberationResult`, `Orchestrate`, council/agent CRUD,
  `ProcessTriggerEvent`, `GetStatus`/`GetMetrics` only
- the contract-shaped surface today is "set
  `Constraints.output_contract` on a generic `DeliberateRequest`"
- no `RunCouncilDecision` or equivalent dedicated RPC; consumers would still
  have to call `Deliberate` / `ProcessTriggerEvent` to obtain a
  contract-validated decision

Relevant code:

- [`crates/choreo-proto/proto/underpass/choreo/v1/choreo.proto`](../crates/choreo-proto/proto/underpass/choreo/v1/choreo.proto)
- [`crates/choreo-app/src/services/auto_dispatch.rs`](../crates/choreo-app/src/services/auto_dispatch.rs)

#### Deliverables

Add a dedicated RPC for contract-shaped expert councils.

Representative shape:

- `RunCouncilDecision`
  - council id / specialty
  - structured external context bundle
  - output contract id
  - validation mode
  - metadata

Response:

- validated structured winner
- validation outcome summary
- candidate summaries
- trace metadata

#### Acceptance criteria

- consumers can call a dedicated RPC without abusing generic trigger fan-out
- council result is already validated when returned

### Epic 10. Report artifact support

Status: not started

Current state:

- no `Report`, `HumanHandoffReport`, or `IncidentAnalysis` entity in
  `choreo-core`
- no report message in proto
- no report-shape validator in `choreo-adapters`
- closest path today is "structured `OutputContract` returning a JSON
  object" — sufficient for a decision proposal, not for a typed report

Relevant code (none yet — this epic adds new types):

- [`crates/choreo-core/src/entities/`](../crates/choreo-core/src/entities/)
- [`crates/choreo-proto/proto/underpass/choreo/v1/choreo.proto`](../crates/choreo-proto/proto/underpass/choreo/v1/choreo.proto)
- [`crates/choreo-adapters/src/validators.rs`](../crates/choreo-adapters/src/validators.rs)

#### Deliverables

Support a structured report result type suitable for `human-handoff-report`.

Minimum fields:

- executive summary
- incident timeline
- findings
- remediations attempted
- open risks
- recommended human actions
- evidence references

#### Acceptance criteria

- report output is schema-validated
- report output can be persisted or returned without lossy flattening

## Phase 6 — Stack E2E Readiness

### Epic 11. Choreographer stack E2E

Status: partial

Current state:

- `crates/choreo-e2e-runner/src/main.rs` runs four scenarios against a
  real gRPC + NATS stack:
  1. seeded council is visible
  2. `Deliberate` on the seeded specialty returns a winner
  3. `DeleteCouncil` on an unknown specialty returns `deleted=false`
  4. inbound `TriggerEvent` over NATS produces an outbound
     `DeliberationCompleted` carrying the same `correlation_id` and
     `causation_id` (scenario added 2026-05-11, PR #45)
- the stack uses `CHOREO_SEED_SPECIALTIES=triage` with the `NoopAgent`,
  so the council is real-shaped but not real-content
- the docker-compose stack has no `CHOREO_EXECUTOR_KIND=runtime` and no
  provider agent config, so the run does not exercise the runtime
  executor or a provider-backed council

Relevant code:

- [`crates/choreo-e2e-runner/src/main.rs`](../crates/choreo-e2e-runner/src/main.rs)
- [`tests/e2e/docker-compose.e2e.yaml`](../tests/e2e/docker-compose.e2e.yaml)

#### Deliverables

Add a reproducible test proving:

```text
bounded external trigger
  -> context bundle
    -> real council
      -> validated structured result
        -> runtime execution or bounded output
```

This E2E does not need a downstream consumer yet, but it must prove the stack assumptions Choreographer
will bring into any consumer integration.

### Epic 12. Consumer integration smoke prerequisites

Status: not started — blocked by Epic 6 (real agent factory composition),
Epic 9 (dedicated council-decision RPC), Epic 10 (report artifact), and
the missing real-council / runtime legs of Epic 11.

#### Deliverables

A test harness that can later prove:

```text
<consumer> specialist escalation
  -> kernel bundle
    -> choreographer reevaluation
      -> validated remedy proposal or human-escalation decision
```

and:

```text
<consumer> escalation decision
  -> choreographer handoff report
    -> final human escalation
```

This can begin only once the earlier epics are green.

### Epic 13. MCP stdio adapter

Status: foundation done (2026-05-12); distribution slice in flight.

Current state:

- `crates/choreo-mcp` exposes every RPC of `underpass.choreo.v1` as
  a `choreo_*` MCP tool (12 tools 1:1 with the gRPC service).
- JSON-RPC 2.0 over stdin/stdout, no MCP SDK — the wire protocol is
  hand-rolled so it stays in lock-step with the proto contract.
- `ChoreoMcpToolBackend` trait has two impls: fixture (canned
  responses for client wiring) and gRPC (live tonic client with
  optional TLS).
- Field-for-field JSON ↔ proto mappers in `src/grpc/{json_to_proto,
  proto_to_json}.rs` — 100% API respected.
- `StreamDeliberation` buffered into one response (frames array +
  winner extracted from the last `result`-typed frame). MCP stdio is
  sync.
- 6 env vars (`CHOREO_MCP_BACKEND` + 5 `CHOREO_MCP_GRPC_TLS_*`) with
  the same auto-detection pattern as the sibling rehydration-mcp.
- 21 unit tests + workspace clippy clean.

Distribution slice (in flight):

- `scripts/mcp/install-choreo-mcp.sh` — `cargo install --git` wrapper
  with pinned `CHOREO_MCP_BRANCH/TAG/REV` (mutually exclusive).
- `scripts/mcp/choreo-stdio-smoke.sh` — one `tools/call` + grep marker
  for both fixture and live modes.
- `docs/operations/mcp-stdio.md` — canonical user-facing UX.
- `docs/operations/mcp/codex.md`, `docs/operations/mcp/claude-desktop.md`
  — per-client config snippets.
- `crates/choreo-mcp/README.md` — developer-oriented twin.
- top-level `README.md` link to `docs/operations/mcp-stdio.md`.

Relevant code:

- [`crates/choreo-mcp/`](../crates/choreo-mcp/)
- [`docs/operations/mcp-stdio.md`](./operations/mcp-stdio.md)

#### Deliverables (open)

1. `crates.io` publication. Blocked by the proto tree being path-deped
   from `choreo-mcp`. Needs the proto package vendored into a
   standalone crate (or `choreo-proto` itself published) before
   `cargo install` from a registry will work.
2. Real-kernel integration test that boots a choreographer in a
   container and exercises every tool through MCP (separate from the
   existing e2e-runner gRPC scenarios).

## Proposed execution order

### Milestone A — real execution and context

Must finish:

- Epic 1
- Epic 2
- Epic 5

Exit condition:

- Choreographer is no longer an isolated deliberation prototype

**Cleared 2026-05-11.** Runtime executor adapter, kernel context
boundary (option A), and causal metadata model are all done.

### Milestone B — contract-grade councils

Must finish:

- Epic 3
- Epic 4
- Epic 10

Exit condition:

- Choreographer can return consumer-safe structured decisions

**Partially cleared 2026-05-11.** Epic 3 done; Epic 4 has the
format/required/allowed slice but still needs JSON Schema and
report-shape validators; Epic 10 (report artifact) not started.

### Milestone C — production honesty

Must finish:

- Epic 6
- Epic 7
- Epic 8

Exit condition:

- composition, transport, and security claims match reality

**Mostly cleared 2026-05-11.** Epics 6 and 7 done; Epic 8 server side
done, Runtime client TLS leg deferred.

### Milestone D — Consumer-facing surface

Must finish:

- Epic 9
- Epic 11

Exit condition:

- consumers have a clean RPC surface to integrate with

**Open.** Epic 9 not started; Epic 11 partial (4 scenarios but no real
council and no runtime executor wired in the stack).

### Milestone E — integration-ready

Must finish:

- Epic 12

Exit condition:

- it is reasonable to begin consumer integration work

**Open** — blocked by Milestones B (remaining), C, and D.

## Suggested issue breakdown

### Cleared waves (2026-05-11)

- runtime gRPC executor adapter — done
- wire runtime executor in composition root — done
- execution metadata model — done (`TaskMetadata.execution_profile`)
- external context bundle type — done
- structured output mode — done
- incident / run / causation metadata propagation — done

### Open waves

#### Wave 4a — real provider factories — done

- dispatching `AgentFactoryPort` recognising
  `noop`/`anthropic`/`openai`/`vllm` shipped as `DispatchingAgentFactory`
- wired in `compose.rs` behind the existing Cargo features
- env-driven config + per-descriptor `provider.*` overrides
- 10 unit tests in `agents/factory.rs`

Open follow-up: explicit Postgres persistence-rehydration test for a
non-noop descriptor (would require provider credentials in CI; not
strictly required by Epic 6's acceptance, since the dispatcher uses
the existing `RegisterAgentUseCase` path that already has rehydration
tests via `NoopAgentFactory`).

#### Wave 4b — transport honesty — done

- AsyncAPI rewritten to declare plain core NATS pub/sub semantics
  consistent with the current adapter; `stack-gap-analysis.md` §4
  retitled accordingly. JetStream remains the upgrade path if the
  bus-coupling requirement later demands durability.

#### Wave 4c — TLS honesty — partially done

- gRPC server: `GrpcTlsConfig` wired through `ServiceConfig`,
  `EnvConfiguration` validates the mode combinations, `runtime.rs`
  applies `ServerTlsConfig` (server or mutual). Chart template now
  mounts the secret and passes env vars; helm-lint gate 4 asserts
  the rendered manifest for both modes; 6 env-loading unit tests
  pin the validation paths.
- Open follow-up: Runtime gRPC client TLS in `RuntimeExecutor::connect`
  (requires coordinated identity rollout with the runtime team) and
  a Rust integration test that performs a real TLS handshake against
  the choreographer (likely with `rcgen` to generate a self-signed
  cert in-test).

#### Wave 5 — Consumer-facing surface

- add a dedicated `RunCouncilDecision` (or equivalent) RPC backed by
  the structured-output mode
- add JSON Schema validator and a report-shape validator
- add `Report` / `HumanHandoffReport` entity + proto + persistence
- extend the e2e-runner to drive a real council + the Runtime executor

## Gating rule

The following rule should be treated as hard policy:

> No downstream product that requires structured, audited deliberation
> output should depend on Choreographer until Milestones A, B, and C
> are complete.

Status 2026-05-12: Milestone A is complete. Milestone B is one open
epic away (10) plus a partial validator slice (4). Milestone C is
mostly complete (Epics 6 and 7 done, Epic 8 server-side done; the
outbound client TLS leg remains).

Why the rule still stands:

- before B's remaining items, the contract-validated decision surface
  exists but has no typed report and no JSON Schema enforcement;
- before C is fully closed, the outbound TLS posture is asymmetric
  (server is hardened, the Runtime client still uses plain TCP).

## Final recommendation

If only one sentence is carried forward from this document, it should be:

> Choreographer's job is to be a trustworthy stack peer — real Runtime,
> real context, structured outputs, honest transport, agent-callable
> through gRPC and MCP, with stack E2E — and nothing more. Downstream
> products integrate; they are not implemented here.
