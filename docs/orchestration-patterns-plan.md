# Orchestration patterns, event sourcing, observability, parity and memory: the plan

Status: plan and design direction, not an implementation claim. Findings were
checked in the code at `main` = `c7dad9f` (after #37) on 2026-09-15. The
owner took the decisions in §5 on 2026-09-16. Everything in §3 is a proposal
until an ADR accepts it and a gate proves it.

What the owner asked for, in their words and in this order:

1. the broadcaster pattern: inside a ceremony, and between ceremonies that
   run in parallel;
2. sequential versus concurrent orchestration, defined precisely — MADE is
   sequential by construction, concurrency is the open problem;
3. four basic ceremonies shipped as data: group chat, maker-checker loop,
   handoff, magentic — and the composition of patterns;
4. event sourcing, done properly;
5. a memory that is not coupled to KMP;
6. observability that works in the embedded edition, through adapters;
7. parity with the gRPC API: MADE is **local-first**; the API may diverge,
   but it should not;
8. the engineering standard every slice obeys — hexagonal (ports, adapters,
   DTOs, mappers, use cases), full DDD without primitive obsession, SOLID,
   one file one class, 80 % tests — and a **fast CI mode for development**,
   like KMP's, to iterate faster.

Pattern vocabulary follows the Azure Architecture Center guide
[AI agent orchestration patterns][ms-patterns] (2026-02-12 revision) so that
the words mean the same thing here as they do outside this repo.

[ms-patterns]: https://learn.microsoft.com/en-us/azure/architecture/ai-ml/guide/ai-agent-design-patterns

---

## 0. Where MADE stands today

Five audits of the code precede this plan. These are the findings that shape
it; each is checkable at the cited path.

### 0.1 Execution model

- A ceremony is a **single-token state machine**: `CeremonyInstance` holds one
  `current_state` (`crates/made-core/src/entities/ceremony_instance.rs`), and
  `start_step` / `apply_step_result` refuse any step outside it
  (`ceremony_instance/step_execution.rs`).
- Steps of a state run **strictly in declaration order**, one at a time, in
  both drivers: the one-shot `RunCeremonyUseCase` loops `for step_id` with an
  inner repeat loop; the embedded presenter exposes a single `next_step_id`
  (`crates/made-app/src/usecases/ceremony_instance_view.rs`).
- Inside a deliberation every agent call is a sequential `for … .await`:
  proposing, the ring critique (`i` critiques `(i+1) mod N`), validating and
  scoring (`crates/made-app/src/usecases/deliberate.rs`). There is no
  `join_all` in production code. The MCP schema text "Requested parallelism"
  for `num_agents` (`crates/made-mcp/src/protocol/general_schemas.rs`) is
  wrong: it is a council-size cap.
- A step is owned by **exactly one role**, a role maps to **one specialty**,
  a specialty to **one council**. A step cannot address a named agent, a
  cross-specialty subset, or several roles. Only interventions address sets of
  roles, and they dispatch no work.
- Guards are four fixed conditions (`Always`, `AllStepsCompleted`,
  `StepStatus`, `HumanApproval`). Nothing branches on a step's **output**;
  `repeat.until` is the only place an output value drives control flow
  (ADR-010), and it is scoped to one step.
- The domain permits branching (several outgoing transitions per state, unique
  `(from, trigger)`), but the design tool emits linear chains, all seven
  checked-in ceremonies are linear five-state chains, and the Mermaid renderer
  assumes linearity.

### 0.2 Events and persistence

ADR-003 chose "snapshot plus append-only journal plus outbox, **not** event
sourcing", and the code matches that decision exactly:

- The snapshot is the source of truth; every commit rewrites the whole
  `ceremony_instances` row
  (`crates/made-adapters/src/sqlite/ceremony_store/ceremony_unit_of_work.rs`).
- `AuditRecord` has **no payload**: type, ids, sequence, actor, timestamps,
  hashes — never what changed (`crates/made-core/src/entities/audit_record.rs`).
  ADR-006 already lives with the cost ("cannot reconstruct payloads the journal
  never recorded").
- There is no `apply(event)`, no rehydration from records, no replay, no
  projection built from records anywhere in the workspace. `CeremonyInstance`
  is built by `open()` or by `serde` deserialization, nothing else.
- The **outbox is built, conformance-tested and unused**: the only two
  production `CeremonyCommit::new` calls pass an empty message list
  (`crates/made-app/src/services/session_journal.rs`), `PublishOutboxUseCase`
  is wired into no composition root, and no `OutboxTransportPort`
  implementation exists outside a test double.
- **No ceremony event is ever published.** The five bus events are council
  events, published synchronously from `DeliberateUseCase` /
  `OrchestrateUseCase`, bypassing the outbox.
- Optimistic concurrency is real (revision CAS inside `BEGIN IMMEDIATE`,
  `CommitOutcome::Conflict`), but **no use case retries**; a conflict
  surfaces to the caller as `DomainError::Conflict`. The revision lives in
  the storage record, not in the aggregate, by design.
- The transcript is appended **outside** the commit transaction and the
  deployable server always wires the in-memory transcript store, even when
  ceremony state is durable (`crates/made/src/compose.rs`).
- Two unguarded write paths survive alongside the unit of work
  (`CeremonyInstanceRepositoryPort::save`, `AuditJournalPort::append`), the
  `state_migrations` table is created and never touched, and
  `AuditChain::verify` is called by no production code path.
- `correlation_id`, `causation_id` and `trace_id` on every ceremony fact are
  hard-coded to `None` (`crates/made-app/src/services/session_facts.rs`),
  and `trace_id` still participates in the record digest.
- What is solid and worth keeping: derived, stable event ids
  (`session_facts.rs`); the two-commit step protocol (claim durable before
  the handler runs); the SHA-256 hash chain with a domain separator and a
  stateless verifier (`audit_chain.rs`); five conformance suites that already
  drive concurrent appends and commits.

### 0.3 Memory

- The memory ports (`MemoryWriterPort`, `MemoryReaderPort`), their value
  objects and the ten-property conformance suite name no kernel, no tool and
  no JSON key. Two non-KMP implementations pass the suite today
  (`InProcessSessionMemory`, `ForgetfulMemory`).
- **The engine writes memory and never reads it.** `MemoryReaderPort` has no
  consumer outside the conformance suite; `AnsweringQuestions`,
  `MemoryQuestion` and `MemoryDimension` are declared by no implementation and
  read by nobody.
- The KMP adapter (`crates/made-adapters/src/kmp/`) sits behind an empty Cargo
  feature, is selectable by no configuration, and is wired into no binary.
  Both composition roots ship `ForgetfulMemory`. The adapter launches
  `rehydration-mcp` with `REHYDRATION_MCP_*` variables — names from before
  the KMP rename — and decides idempotency by substring-matching the kernel's
  English refusals. KMP still aliases `kernel_*` tool names, but the binary
  name and the environment are stale.
- Memory scope is `ceremony:{id}`, so even a working adapter would never let
  one ceremony recall another.
- Residue of KMP inside the domain: a doc comment on
  `MemoryRelationKind::Authorizes` citing a KMP issue number, `"kmp: …"`
  prefixes in `DomainError` reasons, `kmp_*` span names.
- Memory writes are already shaped as a projection
  (`services/session_memory_projection.rs` + `session_memory_recorder.rs`),
  fire-and-forget after commit.

### 0.4 Editions and parity

Four surfaces expose the engine: the gRPC contract (35 RPCs in
`crates/made-proto/proto/underpass/made/v1/made.proto`), the MCP server on the
gRPC backend (35 tools, a 1:1 bijection with the RPCs pinned by
`crates/made-mcp/src/protocol/tests.rs`), the MCP server on the embedded
backend (23 tools), and the Rust facade (`EmbeddedMade`, 31 methods, of which
`made-api`'s `CeremonyEngineApi` publishes 8, read-only plus `start`).

- **Shared by both MCP backends: the 19 ceremony verbs.** Every human-driven
  verb (guards, interventions, evidence, reasons, participant binding) exists
  as RPC, as a tool on both backends and as a facade method.
- **Embedded-only, no RPC: four tools** — `made_design_ceremony`,
  `made_claim_ceremony_step`, `made_complete_ceremony_step`,
  `made_generate_ceremony_report`. The claim/complete use cases exist in
  `made-app` and have no gRPC handler. The audit journal and the transcript
  have no read RPC at all. History shows the pattern: every host-collaboration
  affordance landed embedded-only and never caught up (`ccc0997`, `47a1f44`,
  `a560d34`).
- **gRPC-only, no embedded path: sixteen tools** in three clusters —
  deliberation (`Deliberate`, `StreamDeliberation`, `GetDeliberationResult`,
  `Orchestrate`, `ProcessTriggerEvent`, `RunCouncilDecision`), configuration
  (councils, agents, contracts) and observability (`GetStatus`,
  `GetMetrics`). `docs/editions.md` says so ("not claimed").
- **Three live divergences inside the shared verbs**: `made_run_ceremony`
  omits `steps[].iteration` in the embedded presenter
  (`embedded_run_ceremony_presenter.rs`) while the gRPC mapper emits it — the
  repeat-until commit `0e05aad` updated one presenter and not the other;
  `made_list_ceremony_instances` returns `{count, instances[]}` plus
  `rehydratable: false` entries on embedded and `{instances[]}` on gRPC;
  `made_run_ceremony_step` runs a real deliberation in the cluster and a
  no-op handler in the embedded default, with different default lease owner
  ids. Error envelopes differ (`"gRPC {code}: {msg}"` vs free text).
- **The parity test proves less than its name.**
  `crates/made-tests-integration/tests/mcp_backend_parity.rs` compares the
  key paths and leaf types of `made_get_ceremony_instance` only, skips the
  open-ended fields (`output`, `details`, `context`, `evidence_pack`) where
  the noop and deliberating handlers differ, and checks tool availability
  against a hard-coded allowlist of 17 names — it cannot notice a tool added
  to one side. No request-side schema parity exists for the embedded arm.
- `docs/operations/support-matrix.md` has no edition row at all: parity is
  not a support claim today.

### 0.5 Observability without event sourcing

Observability rests on three legs, and **none of them is the journal**:

- **Metrics at the edges.** A 22-method `MetricsRecorderPort`
  (`crates/made-core/src/ports/metrics_recorder.rs`) with 21 Prometheus
  families (`crates/made-adapters/src/metrics/prometheus_recorder.rs`).
  Cost, latency, saturation and error class are recorded in the provider and
  judge adapters (an RAII guard in `agents/instrument.rs`). Deliberation
  quality is recorded at four points of `deliberate.rs`. The five ceremony
  families are recorded from **one file only**, the one-shot
  `run_ceremony_use_case.rs`; no other ceremony use case has a metrics field.
- **Spans on use cases, narrative only in one.** `#[tracing::instrument]` on
  33 use cases and 36 gRPC handlers. The six span events that carry the
  debate live inside `deliberate.rs`. A ceremony step, transition, guard or
  intervention is a span with ids in the step-at-a-time path and **neither a
  span nor an event** in the one-shot path (`run_step` has no span). Provider
  and judge adapters emit no spans. Export is OTLP over gRPC with optional
  mTLS behind the `otel` feature of the `made` binary
  (`crates/made/src/telemetry.rs`).
- **A call-scoped observer for one RPC.** `DeliberationObserverPort` feeds
  `StreamDeliberation` with phase frames and no payload. There is no ceremony
  observer, no ceremony stream.

What follows:

- **The journal is not an observability source.** You cannot go from a
  journal record to a trace. The embedded report renders every content
  section from the **snapshot** and uses the journal only for the timeline
  (type, actor, order).
- **The step-at-a-time API — the one the local edition uses — records zero
  metrics and emits zero events in every edition.** Metrics exist for the
  `RunCeremony` RPC alone.
- **The embedded edition has no observability.** `EmbeddedMade::open` never
  injects a recorder (`NoopMetricsRecorder` by default), `made-mcp` has no
  exporter and a dead `opentelemetry` dependency, `made_get_metrics` and
  `made_get_status` are gRPC-only, and the default log filter
  `made_mcp=info,made_adapters::sqlite=info` drops every `made_app` span and
  event. What ships is a `made_mcp_tool` debug/warn pair per tool call and
  the payload-free journal chain.
- **The docs promise more than the code does.** Twelve claims in
  `docs/made-observability-design.md` and
  `docs/operations/observability-runbook.md` have no code behind them: gRPC
  RED, Postgres query latency, phase durations, proposals/revisions
  histograms, validator metrics, step attempts, trace exemplars,
  provider/judge span attributes, a readiness gauge that still reads NATS
  only, a summary metric marked "replace" and still shipped, a span tree that
  draws `prepare_ceremony_participants` as a child when it is a sibling, and
  "each event carries `ceremony_id`, `step_id`, `specialty`" which is true for
  one of six messages. Alerts and dashboard rows reference series that do not
  exist. `made_get_metrics` on the gRPC backend returns the five legacy
  counters, never the registry.

### 0.6 Engineering gates and the developer loop

- **What is gated today.** `scripts/ci/architecture-gate.sh` enforces the
  inward dependency table per crate, forbids infrastructure crates in
  `made-core`, one primary public type per source file, a 600-line ratchet
  per production file, and counts public primitive fields in `made-core`;
  the baseline `docs/architecture/conformance.tsv` has 0 debt entries and
  may only shrink. `scripts/ci/rust-coverage.sh` enforces 80 % of production
  lines **workspace-wide** with `cargo llvm-cov` (no per-crate floor).
  `scripts/ci/domain-vocabulary-boundary.sh` keeps vertical vocabulary out
  of the core, the API, the app and the MCP server. Clippy runs with
  `-D warnings` on the full provider matrix.
- **What is review-only.** DTOs and mappers at the edge, value objects
  across domain APIs, and port narrowness are rules in
  `docs/architecture/hexagonal-target.md` and PRINCIPLES §7, enforced by
  review and by the primitive-field ratchet, not by a dedicated check.
- **The developer loop.** MADE's `dev-loop.yml` is a branch trigger
  (`develop`) running the architecture gate, `fmt`, `clippy --lib` and
  `cargo test --lib` on the whole workspace, while the full
  `quality-gate.yml` runs on every pull request to `main`. KMP's loop is
  different in kind: it runs on **draft** pull requests only, over the crates
  named in `DEV_PACKAGES`, and the full gate stands down while the PR is a
  draft and wakes on `ready_for_review`; an **impact planner**
  (`quality-gate-plan.py`) routes only the gates a change can affect, derived
  from the reverse workspace dependency closure, failing closed to the full
  matrix on unknown paths; and `tree-already-proved.sh` skips the gate on a
  merge to `main` whose tree was already proved green. MADE has none of the
  three.

---

## 1. Vocabulary: the five patterns, stated for MADE

The definitions come from the Azure guide; the right-hand columns say what
MADE has and what it lacks. A pattern is a **shape of coordination**, not a
type: ADR-001 rules that ceremony patterns are data, never types in
`made-core`, and this plan keeps that rule.

| Pattern | Coordination | Routing | MADE today | Missing |
|---|---|---|---|---|
| **Sequential** (pipeline, prompt chaining) | Linear; each stage consumes the previous output | Deterministic, predefined | The ceremony FSM with `see_prior` transcript threading; bounded `repeat` (ADR-010); guards; human guards | Nothing. This is MADE by definition |
| **Concurrent** (fan-out/fan-in, scatter-gather) | Several agents work on the same input independently; results aggregated by vote, weighted merge or synthesis | Deterministic or dynamic selection of agents | One council per step, invoked one agent at a time | Concurrent steps in one state; a join; parallel agent invocation; an aggregation strategy; a parallelism cap |
| **Group chat** (roundtable, debate, council) | Shared accumulating thread; a chat manager decides who speaks next; agents read-only; humans observe or manage | Manager-controlled turn order | The council deliberation is a fixed-topology group chat (propose, ring critique, validate, score); the transcript is a shared thread | Manager-selected speaker; a loop over several steps; termination decided by the manager, capped |
| **Maker-checker** (evaluator-optimizer, reflection loop) | Maker proposes, checker judges against declared criteria, feedback loops back until pass or cap | Formal turn-taking | The Validating phase and `output_contract` are a one-shot checker; critique rounds are peer rework, not a checker | A loop over a `make` + `check` pair with a cap and a declared fallback (escalate) |
| **Handoff** (routing, triage, delegation) | One active agent at a time; the agent decides to finish or transfer control | Dynamic, decided by the agent | Roles, transitions with triggers, interventions, human escalation | Transition chosen from the step **output**; a handoff cap; loop detection |
| **Magentic** (task ledger, adaptive planning) | Manager builds and revises a task ledger, delegates, checks progress, stalls detected | Manager assigns and reorders dynamically | Fixed, immutable, analysed definitions (ADR-005) | A ledger carried in context; a bounded execute loop; delegation by ledger entry; stall guard |

Two definitions fixed in writing, because they decide the design:

**Sequential orchestration in MADE** is one token moving through a directed
graph of states; inside a state, steps execute in declaration order; each
step's winner is appended to a transcript the next brief sees. The choice of
the next agent is never the agent's. Loops exist only as bounded `repeat`
(ADR-010) and guarded transitions. This is more than a pipe: it has guards,
human gates, retries and leases. It is still sequential because at any
instant **exactly one step of exactly one state** may be in progress.

**Concurrent orchestration in MADE** means: a state declared
`execution: concurrent` whose steps are all claimable at once, each executed
by its own role and council, each with its own lease, events and repeat
policy; the state's exit guard is the join. The token still sits in one
state. Aggregation is a step of the next state that sees every sibling
output. Concurrency is therefore **between steps of one state**, never between
states. Several active states — statecharts' orthogonal regions, Petri-net
places — are out of scope: they change the aggregate's core invariant, every
view, the diagram, the design tool and the one-shot driver at once, and the
patterns in §3.4 do not need them.

**Broadcaster**, which the Azure guide does not name but the owner asked for:
one producer, many independent consumers, no reply awaited at send time,
ordered per ceremony, at-least-once with idempotent consumers. In MADE it is
the event stream leaving the engine (§3.1) read through cursors (§3.3), and,
inside a ceremony, a concurrent state whose steps share one brief.

---

## 2. Rules this plan obeys

- **Patterns are data** (ADR-001). Each ceremony pattern ships as a YAML
  fragment plus a design-tool preset. The engine gains only generic
  primitives, each usable by any pattern. No `GroupChatState`, no
  `HandoffTransition` in `made-core`.
- **The engine owns contracts, the host owns durability** (ADR-003, kept).
  New ports come with a conformance suite before a second adapter exists.
- **Events are the source of truth for a ceremony** (decision 2, §5). The
  aggregate is a fold over its stream; everything else — instance view,
  transcript, report, memory, metrics, traces, published events — is a
  projection of that stream. Snapshots are a cache.
- **Every loop is bounded** (ADR-010). Every new loop — state repeat, handoff
  chain, magentic execute loop — declares its cap in the definition, and the
  analysis rejects a definition without one.
- **Nothing is claimed without a gate** (PRINCIPLES §1–2). Each slice below
  names the test or experiment that would fail if the claim broke.
- **One aggregate, one token.** `CeremonyInstance` stays the only writer of
  ceremony state; `current_state` stays a scalar.
- **The host performs the work in the embedded edition.** Concurrency there
  means the host may hold several leases; MADE never spawns anything on the
  host's behalf.
- **Local-first with API parity.** The embedded edition is where capability
  is designed and proven first, and the same capability reaches the gRPC
  contract, both MCP backends and the facade in the **same PR**. Divergence
  is allowed only as a named row in the checked-in exception list (§3.6),
  whose target size is zero.
- **Observability is a projection, delivered by adapters.** Ports stay in
  the core; the cluster and the embedded edition differ only in which
  adapters are wired, never in what is observable.

### 2.1 Engineering standard, applied to every slice

The owner's constraints, each with the gate that enforces it and what this
plan adds so that the new code cannot fall below it.

| Constraint | Enforced today by | What the plan adds |
|---|---|---|
| **Hexagonal**: ports, adapters, DTOs, mappers, use cases | Dependency table and forbidden-infrastructure check in the architecture gate; the hexagonal-target rules | Every new port (`CeremonyEventStorePort`, `CeremonySnapshotStorePort`, `CeremonyEventSubscriberPort`, the memory reader consumer) ships as: trait in `made-core/ports`, DTOs split from the trait, a conformance suite, an in-memory adapter and one durable adapter, mappers at the transport edge (proto, MCP JSON, `made-api` views). No use case learns a transport or a store. |
| **Full DDD, no primitive obsession** | Public primitive fields counted in `made-core` (ratchet); PRINCIPLES §7 | New concepts are value objects: `StreamVersion`, `GlobalPosition`, `CursorName`, `EventSchemaVersion`, `FanOutWidth`, `MaxTransitions`, `MemoryScopeInput`. Event payloads reuse the aggregate's value objects; commands are typed inputs, never `serde_json::Value`. `decide` returns domain events; invariants live in the aggregate, not in use cases. |
| **SOLID** | Narrow ports and extension-by-adapter are rules in PRINCIPLES §7 | Ports stay one-role: the subscriber port has one method; metrics, tracing, log and stream adapters are separate implementations behind one dispatcher, so adding a sink adds a file, not an edit. Pattern fragments extend the design tool without touching the engine. |
| **One file, one class** | One-primary-type rule and the 600-line ratchet in the architecture gate (0 debt entries) | `decide` / `apply` land as new submodules under `ceremony_instance/` (`decisions/`, `fold/`), one event type per file under `entities/ceremony_events/`, one projection per file. The baseline stays at zero; a slice that adds debt does not merge. |
| **Tests at 80 %** | Workspace-wide 80 % of production lines in `rust-coverage.sh` | Per-crate floors with a ratchet (as KMP did in its #417), so a new crate cannot hide under the workspace average; conformance suites and property tests (fold equality, random command sequences) count as production coverage; every slice's gate in §3 is a test that would fail if the claim broke. |
| **Fast CI for development** | A branch-based `dev-loop.yml`; no stand-down, no routing | WS-H (§3.9): KMP's draft-PR loop, impact planner and tree-proof, ported. |

---

## 3. Workstreams

Dependencies fix the order. The event stream (A) is the floor under
observability (G), broadcasting (C) and concurrency (B); the patterns (D)
need B plus a handful of generic primitives; parity (F) is a gate that every
other slice must pass; memory (E) is independent and runs beside A.

```text
                 ┌─► G observability (projections)
A event sourcing ┼─► C broadcaster (cursors, spawn/join)
                 └─► B concurrent steps ─► D patterns + composition
F parity gate ── applies to every slice above
H developer CI ─ applies to every slice above
E memory ─────── independent
```

### 3.1 WS-A — Event sourcing, done properly

The owner's decision: a ceremony **is** its event stream. This supersedes the
one sentence of ADR-003 that said "not event sourcing" and the paragraph of
ADR-009 that described a store as "state plus a journal of what happened to
it". Everything else in ADR-003 survives and gets stronger: the engine owns
the contract, the host owns durability, the hash chain is the tamper-evidence
layer, conformance is part of the contract — and now the chain covers what
happened, not only that something did.

#### The model

| Element | Design |
|---|---|
| **Events** | `CeremonyEvent`, one variant per fact the engine already names (`AuditEventType` has 18): `CeremonyStarted`, `ParticipantsBound`, `StepClaimed`, `StepCompleted`, `StepFailed`, `TransitionApplied`, `GuardApproved`, `GuardDeferred`, `InterventionRequested`/`Responded`/`Closed`, `EvidenceCollected`, `ReasonAsserted`, `ContextWritten` (new, §3.5), `CeremonyCompleted`, `CeremonyFailed`, `CeremonyCancelled`; later `ChildSpawned` / `ChildFinished` (§3.3). Each variant carries its full payload (the step output, the transition trigger and states, the guard decision, the intervention text, the evidence pack, the bindings) and its own `schema_version`. Payloads are the value objects the aggregate already uses; no new vocabulary. |
| **Aggregate** | `CeremonyInstance` splits every mutator into `decide(command, &definition) -> Result<Vec<CeremonyEvent>, DomainError>` (pure; all invariants, leases, idempotency and authorization live here) and `apply(&event)` (pure, infallible, no validation). `rehydrate(definition, events)` is the fold. The in-snapshot history lists (`transitions`, `guard_approvals`, `guard_deferrals`, `reasons`, `step_record_history`) become derived state, folded from events. |
| **Stream** | One stream per ceremony: `(ceremony_id, sequence)`, sequence from 1, contiguous. The sealed envelope is today's `AuditRecord` with the event inside: `event_id` (derived, as `session_facts` already does), `event_type`, `schema_version`, ids, `occurred_at`, `actor`, `correlation_id`, `causation_id`, `trace_id` (all filled, §3.7), `previous_record_hash`, `record_hash` over the canonical envelope **including the payload**. `AuditChain::verify` keeps working unchanged and now verifies content. |
| **Concurrency** | `append(stream, expected_version, events)` where `expected_version` is the stream length the command was decided against. A stale expectation returns `Conflict` and writes nothing. The use-case layer retries commutative commands (reload = fold the new events, re-decide, re-append; bounded attempts) and fails fast on non-commutative ones (transition, start). Two hosts completing two steps of a concurrent state therefore both land. |
| **Idempotency** | The store rejects a duplicate `event_id` within a stream; `decide` rejects a command whose derived event id already exists in the fold (this replaces the unbounded `idempotency_keys` set with a property of the stream). |
| **Snapshots** | Optional, keyed by `(ceremony_id, version)`, taken at terminal events and every N events. Load = latest snapshot + events after its version; no snapshot is ever required, and deleting them all changes nothing but speed. A conformance property asserts `snapshot == fold(events[..=version])` on every write. |
| **Projections** | Instance view (already computed), transcript (fold of `StepCompleted`; the transcript store port is deleted), report (fold + definition, fulfilling ADR-006 with payloads), memory (`session_memory_projection` consumes events instead of use-case call sites), observability (§3.7), published events (§3.3). Every projection is a consumer of one `CeremonyEventSubscriberPort` with a durable cursor, or is computed on read. |
| **The outbox is the stream's tail.** | No separate outbox table. A publisher holds a durable cursor (per consumer) over the global order of sealed events and advances it after delivery: at-least-once, ordered per ceremony, idempotent by `event_id`. The seven-property outbox conformance is re-targeted at cursors (claim lease, retry, quarantine survive as cursor semantics). |
| **Storage port** | `CeremonyEventStorePort { append(stream, expected_version, events) -> AppendOutcome; read(stream, from_version); read_all(from_global_position, limit); head(stream) }` plus optional `CeremonySnapshotStorePort`. `CeremonyUnitOfWorkPort`, `CeremonyInstanceRepositoryPort::save` and `AuditJournalPort::append` are removed; reads stay as query ports over the fold. The SQLite adapter keeps the seam: `ceremony_events(k = ceremony_id ‖ 0x00 ‖ BE(sequence), v = sealed event)` with a global position column for cursors, `ceremony_snapshots`, `published_definitions` unchanged. |
| **Definitions** | Published definitions are already immutable and content-addressed; they are a catalog, not an aggregate, and stay as they are. |
| **Council deliberation** | Out of scope for this workstream: `Deliberation` stays a snapshot with its five published bus events until ceremonies are done; noted in ADR-012 as the next candidate. |

#### Migration of existing stores

Existing SQLite stores hold snapshots and payload-less records; the events
cannot be recovered. The migration is copy-on-write, as ADR-008 did for redb:
for every instance, one genesis event `InstanceImported { snapshot,
legacy_journal_head_hash }` opens the stream at the snapshot's revision; the
legacy `audit_journal` and `ceremony_instances` tables are kept read-only as
provenance and the report can still render their timeline. Instances imported
this way carry no per-event payload before the import, and the report says
so. `made-mcp migrate-store` does it once, verifies fold equality against the
imported snapshot before installing, and keeps the original beside the new
file, the way `share-store` did.

#### Slices

| Slice | Change | Gate |
|---|---|---|
| A1 | `CeremonyEvent` and versioned payloads; the sealed envelope carries the event; the digest covers it; readers for `schema_version != 1` (upcasters) with a test fixture per old version. | Chain conformance gains "a record carries its event"; verifier rejects a payload edit. |
| A2 | `decide` / `apply` / `rehydrate` on `CeremonyInstance`; every current mutator becomes a `decide` that yields events and an `apply` that folds them; unit tests fold every event type. | Aggregate tests: for each command, `apply(decide(cmd))` reproduces today's mutator result; property test: fold equality after random command sequences. |
| A3 | `CeremonyEventStorePort` + `CeremonySnapshotStorePort`, in-memory and SQLite adapters, conformance: append-only, contiguous sequence, stale expectation conflicts and writes nothing, duplicate `event_id` rejected, concurrent appends admit one winner, chain intact across commits, `read_all` order stable, fold equality with snapshots. | Both adapters pass; `two_writers_one_store` re-targeted. |
| A4 | Use cases move from `SessionJournal` (load snapshot, mutate, commit snapshot) to `load = fold`, `decide`, `append`, with bounded retry for commutative commands. `session_facts.rs` becomes the event constructors. Correlation, causation and trace ids filled (§3.7). | Every existing use-case test passes unchanged in behaviour; new tests for retry-on-conflict and fail-fast. |
| A5 | Projections: transcript from the fold (port deleted), instance view unchanged, report from events + definition, memory recorder consuming events through the subscriber port. | Report test renders step outputs from the stream alone; memory tests unchanged. |
| A6 | Cursor-based publication replaces the outbox table: `CeremonyEventSubscriberPort` with durable cursors; conformance re-targeted; the old outbox types and table removed. | Cursor conformance; delivery E2E in §3.3. |
| A7 | Migration command and copy-on-write import; dead paths removed (`save`, `append`, `state_migrations`, `migrate_definition_binding`, split in-memory adapters); `made_verify_ceremony_journal` exposed on both surfaces. | Migration test on a fixture store from v0.3.0; architecture gate baseline shrinks. |
| A8 | **ADR-012 (event sourcing)**: events are the source of truth; snapshots are a cache; the chain seals events; the outbox is a cursor; migration is copy-on-write; supersedes the named sentences of ADR-003 and ADR-009. | ADR review. |

### 3.2 WS-B — Concurrent orchestration

| Slice | Change | Gate |
|---|---|---|
| B1 | Definition: a state gains `execution: sequential \| concurrent` (default `sequential`; absent field keeps today's serialized shape). Analysis rules: a concurrent state's steps must belong to distinct roles or declare distinct councils; it needs at least one join guard. Join guards are new generic conditions: `all_steps_completed` (exists), `any_step_completed`, `steps_completed: n` (quorum). No expression language. | Definition analysis tests; YAML round-trip; design-tool schema; proto and both MCP presenters in the same PR. |
| B2 | Aggregate: in a concurrent state several steps may hold live leases at once; `next_step_id` keeps returning the first claimable step and a new `claimable_step_ids` lists all of them (additive on every surface). Per-step `repeat` keeps working per step. Transition out of the state is disabled while any repeat is unmet (ADR-010 rule unchanged). | Aggregate tests: two claims, two completions, any order; conflict-and-retry test over the event store. |
| B3 | Cluster driver: `RunCeremonyUseCase` runs a concurrent state's steps with `join_all` under a semaphore (`max_parallel`, per definition with a config ceiling). Each step keeps its claim-then-complete event pair. Inside a deliberation, `seed_proposals` may run agents in parallel under the same semaphore — **as a separate, measured slice**: experiment `003-parallel-proposing` records latency and provider saturation before it becomes the default. | Compose E2E with a two-role concurrent state; experiment README with numbers. |
| B4 | Embedded driver: the runbook and the `run-ceremony` skill describe fan-out: claim every `claimable_step_ids`, perform with the host's own subagents, complete in any order, then transition. Two hosts sharing the SQLite store may each claim one step. | Embedded stdio test with two claims; two-process test. |
| B5 | Aggregation as step config, not a new handler: the first step of the following state, with `see_prior: true`, sees every sibling output. Optional `aggregate:` block on that step — `synthesize` (a council whose brief lists sibling outputs; the existing handler), `vote` (deterministic majority over a declared output field, no model call). Strategies are data validated by analysis. | Unit tests for `vote`; E2E for `synthesize`. |
| B6 | Observability: fan-out width per state, in-flight per provider (exists), `NoValidProposal` per sibling — all as projections of events (§3.7). Fix the `num_agents` description. | Metrics registry test. |

Definition of done: a `concurrent-review` example ceremony (three reviewers on
one draft, one synthesizer) runs end to end in both editions, appears in the
Mermaid diagram as a fork/join region, and can be emitted by
`made_design_ceremony`.

### 3.3 WS-C — Broadcasting

Three levels, each built on the previous, all on the stream of §3.1.

| Level | What | Depends on |
|---|---|---|
| C1 | **Inside a ceremony.** A "broadcast step" is a concurrent state whose steps share one brief and fan to N roles; the join guard collects. This is WS-B with a fragment, nothing new in the engine. Ships as the `broadcast_collect` fragment (§3.4). | B1–B5 |
| C2 | **Ceremony → world.** Every sealed event leaves through a cursor consumer. Cluster: a NATS publisher on `made.ceremony.*` subjects, declared in the AsyncAPI spec with the event schemas. Embedded: MCP has no reliable server-to-client push for this, so the first transports are **pull** — `made_read_ceremony_events(cursor)` with a durable cursor per consumer name — and a **file sink** (JSON lines the host can tail). Consumers: dashboards, other ceremonies, hosts, evaluators. The five council events move behind the same mechanism. | A1, A6 |
| C3 | **Ceremonies in parallel that talk to each other.** A parent step declares `spawn:` (published definition name/version, one child per entry, child inputs from parent context, `max_children`, `max_depth`); the engine starts the children, records `ChildSpawned` events with their ids, and a generic join guard `children_completed` (all / any / quorum) becomes true as `ChildFinished` events arrive — delivered by the cursor consumer in the cluster and by the host's pull loop in the embedded edition. Children broadcast; the parent subscribes. | B, C2 |

### 3.4 WS-D — The patterns, as data, and their composition

Each pattern is a **fragment**: a reusable, parameterised group of states,
steps, transitions, guards and roles under `api/examples/ceremonies/fragments/`,
with a `pattern:` preset in `made_design_ceremony`, an entry in the blueprint
catalog and an E2E scenario. The generic primitives they need are listed once
in §3.5 because none of them belongs to a single pattern.

#### D1 Group chat orchestration

- **Shape.** State `discussion` with `repeat` at state level: step `manage`
  (role `chat_manager`) reads the thread and outputs
  `{next_speaker, done, instructions}`; step `speak` is bound to the role
  named in `next_speaker` and contributes to the thread. Repeat until
  `manage.output.done == true`, `max_iterations` declared. State `record`
  emits minutes. The transcript is the shared thread.
- **Who manages.** An agent by default; a human manager is expensive (one
  human guard per turn), so the human seat is an **observer** through
  interventions targeted at the table, plus a final human guard.
- **Caps.** The Azure guide recommends three or fewer agents; the analysis
  warns above three roles in a group-chat state.
- **Without new primitives** a v0 exists today: fixed round-robin over N speak
  steps in one state, each with `see_prior`, plus one step repeat with
  `until: done`. Ship it first as `roundtable_fixed_order`; the
  manager-selected version follows the primitives.
- **Gate.** E2E where the manager ends the chat before the cap; E2E where the
  cap fires and the fallback state is reached.

#### D2 Maker-checker loop

- **Shape.** One state with two steps, `make` (role `maker`) and `check`
  (role `checker`), under a state-level `repeat` until
  `check.output.approved == true`, `max_iterations` declared. The checker's
  acceptance criteria are its `output_contract` (exists) plus a prompt that
  must produce `{approved, findings}`. The maker's next iteration sees the
  findings through the transcript. On cap: a transition guarded by
  `step_repeat_exhausted:check` to an `escalate` state with a human guard,
  or a `deliver_with_warning` terminal state — the definition declares which.
- **What MADE already has.** The council's Validating phase is a one-shot
  checker; critique rounds rework a peer's draft. Neither loops with declared
  criteria. The fragment's description states the distinction.
- **Gate.** E2E passes on iteration 2; E2E exhausts and escalates; a
  definition without `max_iterations` is rejected by analysis.

#### D3 Handoff orchestration

- **Shape.** One active role at a time. Every working step outputs
  `{handoff_to: <role> | null, resolved: bool}`. Transitions out of the state
  are guarded by `output_field:<step>:<field>=<json>` — one edge per target
  role, one edge to `resolved`. Each target state has one step bound to the
  target role, with `see_prior` so the receiver gets the whole thread. A
  human seat is a role like any other, reached by the same handoff and
  blocked by a human guard.
- **Caps.** `max_transitions` declared per definition (the one-shot driver's
  `states + transitions + 1` ceiling is a safety net, not a policy). Loop
  detection: the analysis warns on a cycle without a human state; the
  instance refuses the same edge more than `max_bounces` times.
- **Naming.** The blueprint catalog's "Escalation and Handoff Meeting" is a
  meeting *about* transferring work between systems, not this pattern; the
  catalog gets a cross-reference.
- **Gate.** E2E triage → specialist → resolved; E2E that bounces to the cap
  and lands on the human state.

#### D4 Magentic orchestration

- **Shape.** The ceremony is fixed; the **plan is data**. State `plan`: the
  `manager` role writes a `TaskLedger` output object (tasks, owner roles,
  status, goals) that is promoted into context. Optional human guard: review
  the plan before execution. State `execute` with state-level `repeat`:
  `pick_next` (manager selects the next open task and its owner), `do_task`
  (handoff to that owner by D3's mechanism; in the embedded edition the host
  performs the tools, in the cluster the step may call `ExecutorPort`),
  `update_ledger` (manager rewrites the ledger into context, sets `done` or
  `stalled`). Repeat until `done`, `max_iterations` declared;
  `stalled == true` guards a transition to a human state. State `record`
  emits the ledger and the audit trail.
- **Why not dynamic definitions.** ADR-005 makes definitions analysed and
  immutable, and publication binds a digest. A ledger in context keeps that
  and still lets the plan change every iteration. Dynamic sub-ceremonies are
  C3.
- **Gate.** E2E with a three-task ledger completed in four iterations; E2E
  that stalls and escalates; the ledger appears in the report from the stream
  alone.

#### D5 Composition of patterns

The Azure guide's advice is to combine patterns per stage rather than force
one workflow into one pattern. In MADE composition happens at three levels,
and none of them adds a type to the core:

1. **Inside a ceremony, per state.** A definition is a sequence of fragments.
   `made_design_ceremony` accepts `stages[].pattern` ∈ {`sequential`,
   `concurrent`, `broadcast_collect`, `group_chat`, `maker_checker`,
   `handoff`, `magentic`} with the fragment's parameters (roles, prompts,
   caps, join rule), splices the fragments in order, and lets the author
   override any generated element. The analysis validates the joins: each
   fragment has one entry state and declared exit triggers; caps are present;
   roles referenced across fragments exist; a cycle has a cap and a human
   exit. A `x-pattern` annotation on each state is metadata for the diagram
   and the report, never a rule the engine reads.
2. **Between ceremonies.** `spawn` (C3) composes published ceremonies: a
   magentic parent whose tasks are concurrent-review children, a handoff that
   escalates into a decision-council ceremony. The child's terminal event is
   the parent's join input.
3. **Inside a step.** A step that runs a council deliberation (exists) is the
   third level: a group chat of fixed topology inside any fragment.

Gate: an example `incident_review` ceremony composed of a sequential intake,
a concurrent analysis, a maker-checker write-up and a handoff to a human runs
E2E in both editions; the diagram shows one region per pattern; the design
tool emits it from `stages[].pattern` alone.

### 3.5 The generic primitives the patterns need

Seven additions to the definition model. Each is an ADR-sized decision, each
serves more than one pattern, each lands on the four surfaces in one PR.

| Primitive | Used by | Note |
|---|---|---|
| `execution: concurrent` on a state + join guards | C1, B, D1 (parallel critics), D5 | WS-B |
| `repeat` on a **state** (all its steps re-run as one semantic iteration; iteration coordinate on the state) | D1, D2, D4 | Extends ADR-010; per-step `repeat` stays |
| Guard `output_field:<step>:<field>=<json>` | D3, D4, D2 (approved) | Generalises the ADR-010 condition vocabulary; ADR-010 asked for "an explicit future contract" — this is it |
| Guard `step_repeat_exhausted:<step>` | D2, D4 | Makes the cap a routable outcome instead of an error |
| Role binding from a context key (`role_from: context.<key>`), with an allow-list of roles per step | D1 (next speaker), D3, D4 | Resolved at claim time against the definition's roles; a role outside the allow-list is a refusal |
| `context_writes` from step output (declared fields promoted into context) | D4 (ledger), D3 (handoff target), D1 (instructions) | Recorded as `ContextWritten` in the same append as `StepCompleted` |
| `max_transitions` / `max_bounces` per definition | D3, D4, any cyclic graph | Analysis rejects a cyclic definition without them |

### 3.6 WS-F — Parity between the local edition and the API

Parity, defined: **the ceremony capability set is the same on the four
surfaces** — proto, MCP on gRPC, MCP on embedded, `EmbeddedMade` (with
`made-api` as its versioned subset) — with identical request schemas,
identical result shapes for the same state, and one error envelope. The API
may diverge only as a named row in a checked-in exception file, and a test
compares that file with the real tool sets. The direction is local-first:
the embedded edition leads, the API follows in the same change.

| Slice | Change | Gate |
|---|---|---|
| F1 | **ADR-014 (parity)** states the definition and the one-PR rule. The exception list is data: `docs/architecture/parity.tsv`, one row per capability, a column per surface, as `conformance.tsv` already does for adapters. | A test asserts set equality between the catalog per backend, the proto RPC list, `EmbeddedMade`'s method list and the TSV. Adding a tool to one side fails CI until the row says so. |
| F2 | **Close the three live divergences**: `steps[].iteration` in the embedded run presenter; `{count, instances[]}` plus `rehydratable` / `reason` on both backends and in the proto; one error envelope (`{code, message, retryable}`, reusing `made-api`'s `ApiError` vocabulary); one default lease owner naming rule. | Shape parity test over every shared tool. |
| F3 | **Bring the embedded-only tools to the API.** `ClaimCeremonyStep` and `CompleteCeremonyStep` RPCs over the existing use cases (the delegated-host protocol then works against a cluster too); `DesignCeremony` as an RPC over the same intent document; `ReadCeremonyEvents` (the stream, with cursor) and `GetCeremonyTranscript` read RPCs, which give `GenerateCeremonyReport` a remote path or let the MCP server render the report on the gRPC backend from those reads. Add the tools to the gRPC backend catalog. | Catalog↔proto bijection test extended; the exception file has no ceremony rows. |
| F4 | **Make the parity test a real gate.** Drive the same session on both backends with the same handler wiring, compare shape **and** values for every shared tool, compare request acceptance through the schema gate with an embedded arm, compare error envelopes. Replace the allowlist with the TSV comparison from F1. | The three F2 divergences, reintroduced on a branch, fail the test. |
| F5 | **The council surface in the local edition.** The sixteen gRPC-only tools are absent from embedded by design today. `GetStatus` / `GetMetrics` come to embedded in §3.7. Deliberation and councils stay cluster-only, listed with their reason in the exception file, until B3's parallel proposing lands — the first feature the local edition would otherwise never see — at which point they come into `EmbeddedMade` behind the provider features (`made-embedded` already excludes only `tonic`, `async-nats` and `sqlx`, not the HTTP provider clients). | The exception file names each with its reason; a later slice empties it. |
| F6 | **Support matrix gains an "Editions" section**: one row per capability group with the surfaces it is supported on and the gate that proves it. `docs/editions.md` "tool surfaces differ by design" becomes a pointer to that table. | The table is checked against `parity.tsv`. |

`made-api` stays a versioned subset by ADR-004; parity does not require the
contract to expose mutations. It does require that the facade behind it has
every verb the RPCs have, which F1's test keeps true.

### 3.7 WS-G — Observability, embedded included, through adapters

With A1 the stream is complete; ceremony observability becomes a projection
of it. The ports live in the core; each edition wires the adapters it can
run. Nothing observable depends on the edition.

| Slice | Change | Gate |
|---|---|---|
| G1 | **One seam.** A `CeremonyEventSubscriberPort` consumer (sync, infallible for the metrics and tracing adapters, like `MetricsRecorderPort` today) receives every sealed event after append. Adapters: **metrics** (the five ceremony families plus the missing ones — step claimed, attempt, iteration, guard decided, intervention opened/answered, transition applied, lease acquired/expired, fan-out width), **tracing** (a span event per ceremony event with its payload fields, under the span that appended it), **structured logs** (one JSON line per event at `info`). The thirteen hand-placed metric calls in `run_ceremony_use_case.rs` are removed once the seam reproduces them. | Same session through the one-shot and step-at-a-time drivers yields identical metric deltas; the registry test lists every family with its source event. |
| G2 | **Fill the trace.** Events take `trace_id`, `correlation_id` and `causation_id` from the current span context (`tracing_opentelemetry` on the server; a per-tool-call trace id minted by `made-mcp` when the host sends no `traceparent`, which MCP allows through request metadata). `made_get_ceremony_instance`, `made_read_ceremony_events` and the report expose them. | Chain conformance: "a record carries the trace that wrote it"; report test shows the id. |
| G3 | **Embedded adapters.** `EmbeddedMade::open` wires `PrometheusMetricsRecorder` with an in-process registry; `made_get_metrics` and `made_get_status` join the embedded backend and return the rendered registry (text and a JSON projection) — the gRPC backend returns the same registry instead of the five legacy counters. `made-mcp` gains the `otel` feature and honours `MADE_OTLP_ENDPOINT` and the mTLS variables exactly as `made` does (the dead `opentelemetry` dependency either serves that or leaves); a **file sink** adapter writes events and metrics snapshots as JSON lines to a host-chosen path; the default log filter becomes `made_mcp=info,made_app=info,made_adapters::sqlite=info`. A Rust host injects the same adapters through the builder. | Embedded stdio test reads a ceremony counter after one step; `real_kernel` discovery shows `service_observability` on both backends; a test tails the file sink. |
| G4 | **Spans where the runbook says they are.** A span per step in the one-shot driver (`run_step`), a span for the step handler, spans on the provider and judge adapters carrying `provider`, `model`, `error_kind` and token counts. | Trace-shape test against an in-memory exporter. |
| G5 | **Docs truth pass**, first. Every claim in `docs/made-observability-design.md` and `docs/operations/observability-runbook.md` without code behind it is either scheduled in G1–G4 by name or deleted, per PRINCIPLES §1. Alerts and dashboard rows that reference absent series go with them or move to a "planned" section. | Docs review with the §0.5 list as the checklist. |
| G6 | **Ceremony progress as a stream.** `StreamCeremony` RPC on the cluster fed by the subscriber port; the pull cursor of C2 on the embedded edition. Same events, two doors. | Parity test covers the stream on gRPC and the cursor on embedded against the same session. |

### 3.8 WS-E — Memory that is MADE's own

The bounded-context rule applies: MADE's memory is what a **later ceremony**
needs to know from earlier ones, expressed in MADE's words; a kernel is one
adapter and maps at the boundary with DTOs.

| Slice | Change | Gate |
|---|---|---|
| E0 | Measure the current adapter against `kmp-mcp` 0.18 with the binary name and environment it expects, and record the result under `docs/experiments/`. | A recorded run. |
| E1 | **Add the consumer.** At ceremony start, `MemoryReaderPort::recall(scope)` for the declared scope; the recollection is rendered into the first brief as "what earlier sessions decided" (bounded size, decisions and constraints first). Scope comes from a new definition input `memory_scope`; default stays `ceremony:{id}`, which means "no shared memory", stated as such. | A second ceremony in the same scope sees the first one's decision in its brief. |
| E2 | **Trim the port to what MADE uses.** Remove `AnsweringQuestions` / `MemoryQuestion` and `MemoryDimension` (or give each a consumer in E1); delete the kernel-citing doc comment on `Authorizes` and add the variant to the MCP schema enum where it is missing; replace `"kmp: …"` error prefixes; rename `kmp_*` spans to `memory_*`. Keep the ten-property conformance suite as the contract. | Conformance green; grep gate for `kmp` / `kernel` in `made-core`. |
| E3 | **Ship a durable default.** `SqliteSessionMemory` in the ceremonies store (new tables under the same seam), passing conformance, wired by default in the embedded builder (replacing `ForgetfulMemory`) and in the server behind `MADE_MEMORY=sqlite\|none`. The memory recorder becomes a subscriber of the event stream (A5). | Conformance on SQLite; restart test: a decision survives reopen. |
| E4 | **Move the KMP adapter out of the tree** into its own crate (`made-memory-kmp`, depending on `made-core` only) or into the KMP repository as "MADE adapter", with the conformance suite as its gate. Fix its names there (`kmp-mcp`, `KMP_MCP_*`, `kmp_*` tools) and replace refusal-string matching with structured error codes once KMP exposes them. | `made-adapters` has no `kmp` module; CI matrix drops the `kmp` feature; the external crate's CI runs the suite. |
| E5 | **ADR-013 (memory)**: memory is MADE's own bounded context; recall is a first-class use case; SQLite is the reference implementation; kernels are out-of-tree adapters. Update `docs/index.md`, `stack-gap-analysis.md`, the platform table in `README.md`. | ADR review. |


### 3.9 WS-H — A fast CI mode for development

The target is KMP's loop, ported: minutes of feedback while a change is a
draft, the full gate only when it is ready, and never twice for the same
tree. Nothing merges on the fast loop's word; branch protection keeps every
required check between a ready pull request and `main`.

| Slice | Change | Gate |
|---|---|---|
| H1 | **Draft-PR dev loop.** `dev-loop.yml` triggers on `pull_request` (draft only) and `workflow_dispatch`, replacing the `develop`-branch trigger. `DEV_PACKAGES` names the crates under work for the current phase (phase 1: `-p made-core -p made-app -p made-adapters`). Jobs: `fmt`, `clippy` on `DEV_PACKAGES`, tests of `DEV_PACKAGES`, the architecture ratchet, the parity gate (F1, it is seconds), and a `made-mcp` embedded binary built for the maintainer's machine and uploaded as an artifact. One `rust-cache` key per lane, `cancel-in-progress` per ref. | The loop answers a draft in under ten minutes on a warm cache; a workflow-contract test pins the trigger and the stand-down condition. |
| H2 | **The full gate stands down on drafts and wakes on ready.** `quality-gate.yml` adds `ready_for_review` to its `types` and guards its planner job with `draft == false`; every downstream job needs the planner's outputs, so they all stand down with it. | A draft PR runs only the dev loop; marking it ready runs everything; the required checks list is unchanged. |
| H3 | **Impact planner.** `scripts/ci/quality-gate-plan.py`, ported: reverse workspace dependency closure from `cargo metadata`, path routing for the independent contracts (proto/AsyncAPI, embedded boundary, plugin bundle, chart, container image, coverage, publication dry-run), `--self-test`, unknown paths and edits to the planner itself fail closed to the full matrix; `workflow_dispatch` stays full. | The planner's self-test runs first in the gate; a docs-only PR runs no Rust job; a `made-core` change runs everything. |
| H4 | **Tree proof.** `scripts/ci/tree-already-proved.sh`: a push to `main` whose tree hash was proved green by the merged pull request's gate skips the gate; a merge from an out-of-date branch, a conflict resolved in the UI or a direct push still runs it. | Two consecutive merges: the second, byte-identical tree does not rebuild. |
| H5 | **Local mirror.** `just dev` runs exactly the dev loop for `DEV_PACKAGES`; `just check` stays the full gate; `docs/dev-loop.md` documents both and the draft/ready handover. Per-crate coverage floors with a ratchet join `just coverage`. | `just dev` and the workflow run the same script. |


---

## 4. Sequencing

| Phase | Content | Exit criterion |
|---|---|---|
| 0 | ADR-012 (event sourcing), ADR-013 (memory), ADR-014 (parity) — written 2026-09-16 under `docs/adr/`; G5 docs truth pass; H1–H2 (the draft loop and the stand-down), because every later slice iterates on it. | ADRs merged; the observability docs claim only what runs; a draft PR gets feedback in minutes. |
| 1 | F1, F2, F4 (the parity gate, cheap and prerequisite); H3–H5; A1–A4 (events, aggregate, store, use cases); E0–E3 in parallel. | Parity gate green with a non-empty exception file; impact routing and tree proof live; ceremonies are folds over SQLite-stored streams; conflict retry proven; SQLite memory default. |
| 2 | A5–A8 (projections, cursors, migration, ADR); G1–G3 (seam, trace ids, embedded adapters); C2 (NATS publisher, pull tool, file sink); F3 (API catch-up: claim, complete, design, events, transcript). | Existing v0.3.0 stores migrate; metrics identical across drivers and editions; ceremony events reach NATS and the pull cursor; the exception file has no ceremony rows. |
| 3 | B1–B6 and the §3.5 primitives, each on four surfaces; D1–D4 fragments; D5 composition in the design tool; C1; G4. | `concurrent-review` and `incident_review` run E2E in both editions; four fragments run E2E; diagrams show pattern regions. |
| 4 | C3 spawn/join; G6 ceremony stream; E4, E5; F5 revisited with B3's numbers; F6. | Parent/children E2E over NATS and over the pull cursor; KMP adapter out of tree; support matrix has an editions section. |

Each slice lands as its own PR, iterated as a draft on the dev loop and
merged on the full gate, with: the gate named in its table row, a CHANGELOG
entry under `Unreleased`, the architecture, parity and coverage gates green
(zero new debt, per-crate floors held), the conformance suites green, and the
docs (`editions.md`, runbooks, skills) updated in the same PR when the
operator surface changes.

---

## 5. Decisions

Taken by the owner on 2026-09-16, with the technical consequences this plan
draws from them:

1. **Event sourcing, done properly.** A ceremony is its event stream; the
   aggregate is `decide` + `apply` + fold; snapshots are a cache; the hash
   chain seals events with their payloads; the outbox is a cursor over the
   stream; existing stores migrate copy-on-write. Supersedes the "not event
   sourcing" sentence of ADR-003 (ADR-012). Council deliberation follows
   later.
2. **Parity with gRPC.** The local edition leads; the API may diverge only
   as a named row in `parity.tsv`, whose target size is zero; one PR lands a
   ceremony capability on all four surfaces (ADR-014).
3. **Observability in the embedded edition, through adapters.** Ceremony
   observability is a projection of the stream at one seam; the embedded
   edition wires an in-process Prometheus registry, optional OTLP export
   with the cluster's environment variables, a file sink, and the same
   metrics/status tools as the cluster.
4. **The patterns, as data.** Group chat, maker-checker, handoff and magentic
   ship as fragments plus design-tool presets over seven generic primitives;
   no pattern types in `made-core` (ADR-001 holds).
5. **Broadcasting, concurrency and composition.** Concurrency is between
   steps of one state; broadcasting is the stream read through cursors, plus
   concurrent fan-out inside a ceremony and spawn/join between ceremonies;
   composition is per stage in the design tool, between ceremonies through
   spawn, and inside a step through the council.
6. **Memory is MADE's own.** SQLite reference implementation in tree, recall
   as a use case, KMP adapter out of tree (ADR-013).
7. **The engineering standard is a gate, and development is fast.**
   Hexagonal with ports, adapters, DTOs, mappers and use cases; full DDD
   without primitive obsession; SOLID; one file one class; 80 % coverage with
   per-crate floors; and KMP's draft-PR loop, impact planner and tree proof
   ported so that a change gets feedback in minutes and the full gate runs
   once, when the change is ready.

## 6. Open questions

- State-level `repeat`: a new iteration coordinate on the state, or a
  self-transition with an iteration counter? The first is simpler to present;
  the second reuses transitions.
- Snapshot cadence: every N events, at terminal only, or host-configured?
  Cheap to change later; needs a default.
- One `CeremonyEventSubscriberPort` with a dispatcher adapter, or separate
  ports for metrics, tracing and the stream, as `MetricsRecorderPort` and
  `DeliberationObserverPort` are separate today?
- Where does the external KMP adapter live: a crate in this organisation, or
  a module in the KMP repository that compiles against `made-core`?
- Should `made-api` grow to contract v4 with `claim_step` / `complete_step`
  and `read_events`, or does ADR-004's "mutations stay behind the engine's
  own surfaces" hold for the delegated-host pair too?
- When the council deliberation moves to event sourcing, do its five bus
  events become variants of the ceremony stream (a step that deliberates)
  or a stream of their own?
