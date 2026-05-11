# Choreographer Readiness Backlog For PIR

Snapshot date: 2026-04-25

This document converts the PIR integration design into a concrete technical
backlog for `underpass-choreographer`.

Companion documents:

- [`pir-choreographer-integration-design.md`](./pir-choreographer-integration-design.md)
- [`stack-gap-analysis.md`](./stack-gap-analysis.md)

The goal is not to list every desirable improvement in Choreographer.
The goal is to define the minimum work required before PIR can depend on
Choreographer for complex-incident reevaluation and human handoff synthesis.

## Executive summary

PIR should not integrate with Choreographer yet.

Before PIR can safely depend on it, Choreographer needs to reach a
stack-ready state in eight areas:

1. real Runtime execution
2. real Kernel-fed context input
3. structured, contract-validated council outputs
4. complete incident / causality metadata propagation
5. provider-backed council materialization
6. honest and durable transport semantics
7. real TLS / mTLS posture
8. stack-level end-to-end proofs

The recommended execution order is:

- Phase 1: execution and context foundations
- Phase 2: contract-shaped deliberation
- Phase 3: transport and operational hardening
- Phase 4: PIR-facing integration surface
- Phase 5: stack E2E readiness proof

No PIR integration work should begin before Phases 1 through 3 are complete.

## Out of scope

This backlog does not include:

- replacing PIR's event catalog
- replacing PIR's specialist catalog
- moving kernel graph semantics into Choreographer
- making Choreographer domain-specific to payments incidents
- implementing PIR itself in this repository

## Readiness definition

For the PIR integration described in
[`pir-choreographer-integration-design.md`](./pir-choreographer-integration-design.md),
Choreographer is "ready" when all of the following are true:

- it can consume bounded incident context produced upstream
- it can run real provider-backed expert councils
- it can return structured, validated outcomes with deterministic failure modes
- it can hand off execution to Runtime through a real adapter
- it preserves causal metadata across its full lifecycle
- it operates with honest transport and TLS semantics
- it has at least one reproducible stack E2E against Runtime and Kernel surfaces

## Priorities

### P0 — hard blockers

These items block PIR integration entirely.

- real Runtime executor adapter
- Kernel context boundary
- structured council output contracts
- causal metadata model
- provider-backed agent factory wiring
- truthful transport and TLS posture
- stack E2E proof

### P1 — required before production

These items may start slightly later but must be complete before production use.

- dedicated PIR-facing RPC surface
- contract-aware validators
- structured report artifact support
- release-gate stack smoke

### P2 — useful after first integration

- bus-native PIR ↔ Choreographer coupling
- per-proposal streaming for expert councils
- richer score explainability

## Phase 1 — Runtime And Context Foundations

### Epic 1. Runtime executor adapter

Status: not done

Current state:

- the binary wires `NoopExecutor` unconditionally
- `OrchestrateUseCase` can publish lifecycle events but execution is not real

Relevant code:

- [`crates/choreo/src/compose.rs`](../crates/choreo/src/compose.rs)
- [`crates/choreo-core/src/ports/executor.rs`](../crates/choreo-core/src/ports/executor.rs)
- [`crates/choreo-app/src/usecases/orchestrate.rs`](../crates/choreo-app/src/usecases/orchestrate.rs)

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

Status: not done

Current state:

- Choreographer has no Kernel adapter
- tasks accept opaque `attributes`
- no first-class rehydration path exists

Relevant code:

- [`crates/choreo-core/src/entities/task.rs`](../crates/choreo-core/src/entities/task.rs)
- [`docs/stack-gap-analysis.md`](./stack-gap-analysis.md)

#### Deliverables

1. define one explicit context ingestion boundary:
   - option A: PIR / caller fetches kernel context and passes it to Choreographer
   - option B: Choreographer can fetch context itself through a new port
2. choose one as the production path for the first PIR integration
3. define a stable structured bundle shape for expert councils:
   - incident summary
   - prior findings
   - prior decisions
   - evidence references
   - failed remediations
4. make that bundle addressable and testable

#### Recommendation

For the first slice, prefer caller-materialized context:

- PIR remains the kernel-first owner
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

Status: not done

Current state:

- proposals are free-form text
- validators are generic
- no typed winner schema exists

Relevant code:

- [`crates/choreo-core/src/entities/proposal.rs`](../crates/choreo-core/src/entities/proposal.rs)
- [`crates/choreo-core/src/ports/validator.rs`](../crates/choreo-core/src/ports/validator.rs)
- [`crates/choreo-app/src/usecases/deliberate.rs`](../crates/choreo-app/src/usecases/deliberate.rs)

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

Status: not done

#### Deliverables

Add validators for:

- JSON schema
- required fields
- enum / allowed-decision membership
- bounded event proposal shape
- report artifact shape

#### Acceptance criteria

- validators are composable through the existing validation pipeline
- validator reports remain domain-agnostic from Choreographer's perspective
- PIR-facing council contracts can be enforced with no handwritten
  post-processing hacks

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

Status: not done

Current state:

- `NoopAgentFactory` is wired in the binary
- provider adapters exist but are not composed

Relevant code:

- [`crates/choreo/src/compose.rs`](../crates/choreo/src/compose.rs)

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

Status: not done

Current state:

- docs and specs mention JetStream
- current implementation behaves as plain NATS pub/sub

Relevant code:

- [`crates/choreo-adapters/src/nats`](../crates/choreo-adapters/src/nats)
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

For a critical PIR integration, prefer real JetStream semantics if bus coupling
is expected later. If not, document plain NATS honestly and keep PIR's first
integration on direct gRPC.

#### Acceptance criteria

- code, docs, tests, and spec all say the same thing
- transport semantics are exercised in integration tests

### Epic 8. TLS / mTLS parity

Status: not done

Current state:

- chart exposes TLS knobs
- binary does not back them with real server TLS wiring

#### Deliverables

1. add server TLS/mTLS wiring if the chart surface is kept
2. or remove unsupported chart keys until real
3. ensure client-side TLS exists for Runtime calls

#### Acceptance criteria

- every declared TLS value has a code path behind it
- there is at least one integration test for enabled TLS mode

## Phase 5 — PIR-Facing Integration Surface

### Epic 9. Specialist-grade RPC surface

Status: not done

Current state:

- generic `TriggerEvent` is too weak for PIR integration

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

- PIR can call a dedicated RPC without abusing generic trigger fan-out
- council result is already validated when returned

### Epic 10. Report artifact support

Status: not done

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

Status: not done

#### Deliverables

Add a reproducible test proving:

```text
bounded external trigger
  -> context bundle
    -> real council
      -> validated structured result
        -> runtime execution or bounded output
```

This E2E does not need PIR yet, but it must prove the stack assumptions Choreographer
will bring into PIR.

### Epic 12. PIR integration smoke prerequisites

Status: not done

#### Deliverables

A test harness that can later prove:

```text
PIR specialist escalation
  -> kernel bundle
    -> choreographer reevaluation
      -> validated remedy proposal or human-escalation decision
```

and:

```text
PIR escalation decision
  -> choreographer handoff report
    -> final human escalation
```

This can begin only once the earlier epics are green.

## Proposed execution order

### Milestone A — real execution and context

Must finish:

- Epic 1
- Epic 2
- Epic 5

Exit condition:

- Choreographer is no longer an isolated deliberation prototype

### Milestone B — contract-grade councils

Must finish:

- Epic 3
- Epic 4
- Epic 10

Exit condition:

- Choreographer can return PIR-safe structured decisions

### Milestone C — production honesty

Must finish:

- Epic 6
- Epic 7
- Epic 8

Exit condition:

- composition, transport, and security claims match reality

### Milestone D — PIR-facing surface

Must finish:

- Epic 9
- Epic 11

Exit condition:

- PIR has a clean RPC surface it can integrate with

### Milestone E — integration-ready

Must finish:

- Epic 12

Exit condition:

- it is reasonable to begin PIR implementation work

## Suggested issue breakdown

### Wave 1

- add runtime gRPC executor adapter
- wire runtime executor in composition root
- add execution metadata model

### Wave 2

- define external context bundle type
- define structured output mode
- add JSON schema validator

### Wave 3

- add incident / run / causation metadata propagation
- add dedicated council decision RPC

### Wave 4

- wire real provider factories in the binary
- resolve NATS vs JetStream honesty gap
- resolve TLS server honesty gap

### Wave 5

- add stack E2E for contract-shaped council execution

## Gating rule

The following rule should be treated as hard policy:

> Do not start implementing PIR's `complex-incident-reevaluation` or
> `human-handoff-report` dependency on Choreographer until Milestones A, B,
> and C are complete.

Why:

- before A, Choreographer has no real execution / context posture
- before B, it has no safe structured decision surface
- before C, its transport and security claims are still weaker than the stack
  it wants to join

## Final recommendation

If only one sentence is carried forward from this document, it should be:

> The first job is not to wire PIR into Choreographer; the first job is to make
> Choreographer a trustworthy stack peer with real Runtime, real context,
> structured outputs, honest transport, and stack E2E.

Only after that should PIR depend on it for complex pre-human incident
reevaluation and human handoff synthesis.
