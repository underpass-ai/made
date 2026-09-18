# Changelog

All notable changes to MADE are tracked here.

`v0.1.0` is the first tagged release. Keep new entries under
`Unreleased` until the release process in `docs/release.md` bumps the
version and creates the next immutable tag; released sections are not
edited afterwards.

The format follows the spirit of Keep a Changelog, with categories kept
short and factual. Do not add claims here unless the behavior is
implemented and covered by a committed gate, smoke test, or documented
operator command.

## Unreleased

Phase 3a additions target **0.5.0**. The phase 2 **0.4.0** baseline includes
the documentation closure in #121 and publication review repairs in #126.
Its highlights remain recorded separately here until the release procedure
creates the immutable tagged sections.

### 0.4.0 highlights

- Ceremony event streams are now the source of truth: commands decide sealed
  events, instances fold them, SQLite stores them with a global order, and
  snapshots are optional caches (#42–#45).
- Transcript and memory became projections of sealed records and the transcript
  store port was removed (#83). Copy-on-write migration imports legacy stores,
  then removes the old unit-of-work, whole-state write and outbox paths (#86).
- Session memory is a MADE-owned bounded context with recall, nine-property
  conformance and a durable SQLite reference adapter selected by path-aware
  compositions; the generic builder remains forgetful (#84, #102). MADE ships
  no KMP adapter (#41).
- Durable named cursors now drive explicit pull acknowledgement, NATS
  publication and the embedded JSONL sink; whole and paged event history share
  a typed page limit (#108).
- Automatic publication retries a transient failure without another append.
  Core NATS drains the client buffer under a five-second deadline before
  advancing the cursor; this does not promise remote acknowledgement (#126).
- Metrics, traces and structured logs consume the same sealed records in both
  editions, while reports alone capture one bounded stream cut (#110). The
  shared service-metrics projection preserves legacy statistics and adds
  registry text and structured samples. Embedded hosts can wire OTLP/mTLS and
  paired event/registry JSONL lines through the same recorder; that registry
  snapshot describes the process at delivery time, not an exact historical
  stream prefix (#119).
- The final parity session fills optional fields and rotates enum variants in
  #120 on memory and SQLite, including sealed leases, intact journals and
  all four result labels. The original report golden remains unchanged.

### Remaining 0.4.0 debt

- Shared domain value objects for the list bounds now enforced at MCP ingress
  remain tracked in #100. Whole/paged history, its typed page limit and one-cut
  reports are complete in #108 and #110.

### Added

- Ceremony steps can resolve an allowed role from a top-level context key at
  claim time and seal that role for completion, replay and audit attribution.
  Declared successful output fields can be copied atomically into ceremony
  context through `ContextWritten`; the proto, both MCP backends, YAML and the
  embedded facade share the same validation and serialization. (#130)

- Ceremony definitions can declare positive `max_transitions` and
  `max_bounces` budgets across YAML, proto, direct gRPC, both MCP editions and
  the embedded facade. Cyclic graphs require either cap; transition decisions
  count sealed total and exact-edge history across replay, snapshots and
  reopen, refusing exhausted budgets before append. (#131)

- Ceremony states can declare a bounded repeat-until policy that reruns every
  state step under a durable `state_iteration`. Step iteration and retry
  attempt remain independent; replay, snapshots, history, traces, transcripts,
  direct gRPC, both MCP backends, YAML, and the embedded facade expose the
  coordinate while pre-repeat sealed records retain their hashes. (#128)

- Ceremony definitions can declare concurrent states with `all`, `any`, or
  counted joins and a definition-level `max_parallel` limit. Live reads expose
  every currently claimable step while preserving `next_step_id`; aggregate
  claims and transitions apply the same clock, retry, lease, and host-ceiling
  rules across direct gRPC, both MCP backends, and the embedded facade. (#122)

- `made_design_ceremony` accepts the typed `roundtable_fixed_order` preset on
  the proto, both MCP backends, and the embedded facade. It expands participants
  in declaration order into sequential speaking turns, gives prior context to
  every turn after the first, and ships its discoverable YAML fragment under
  `api/examples/ceremonies/fragments/`. Explicit-stage design remains unchanged.
  (#114)

### Changed

- Ceremony runs now export step and handler spans on every execution surface,
  while provider and judge adapters export concrete identity, stable failure
  class and upstream-reported token usage. Empty or malformed model content no
  longer hides token usage that the provider already reported. (#123)

- Ceremony definitions and `made_design_ceremony` can require exact JSON from
  a declared step's current successful output and explicitly route a bounded
  step-repeat exhaustion. An exhaustion route waives only that step's repeat
  condition on that transition; all other guards and invariants remain in
  force. (#116)

- Automatic ceremony-event publication now retries a transient pending
  position with bounded backoff without waiting for another append. Core NATS
  drains its client buffer to the transport under a five-second deadline before
  advancing the durable cursor, without claiming broker, subscriber or
  JetStream acknowledgement. (#126)

- Align ADR-003, ADR-012, ADR-013, the phase plan, Editions, the platform
  boundary and stack analysis with the verified phase 2 implementation; keep
  future pattern work and the remaining list-bound value-object debt explicit
  (#121).

- Run the complete MCP parity session with filled optionals and all public
  actor, result, reason, reference and confidence variants on memory and SQLite;
  compare status text without hiding non-version differences. (#120)

- `made_get_metrics` now returns the same in-process Prometheus registry as
  text and structured families/samples in both editions while preserving the
  legacy statistics. Embedded hosts can share one recorder/reader adapter,
  export OTLP with the existing mTLS variables, and append registry snapshots
  beside durable JSONL event deliveries. (#119)

- Preserve waiting-for-human and cancelled step-result labels in ceremony
  metrics instead of counting both as failed results. (#118)

- Move ceremony step execution into a child module of the application driver,
  preserving method bodies and all execution behavior. (#115)

- Ceremony telemetry now projects sealed records through metrics, tracing, and
  structured-log subscribers in both editions. One-shot and step-at-a-time
  execution produce the same ceremony metric deltas, and MCP tool calls carry
  one trace through every record they seal. Instance reads, event reads, and
  reports expose trace, correlation, and causation identifiers. Reports fold
  and render one bounded stream snapshot, so a concurrent append cannot mix
  two stream versions in one document. (#110)

- Durable named ceremony-event cursors now drive explicit pull acknowledgements,
  NATS publication on `made.ceremony.<event_type>`, and the embedded JSONL sink
  selected with `MADE_MCP_EVENT_SINK_PATH` (#108).

- MCP sealed records, instance listings, and statistics envelopes now use one
  JSON renderer across the embedded, gRPC, and fixture backends (#101).

### Architecture

- Close the architecture gate blind spots for private primary types, primitive
  fields on application use-case boundaries, and large files with no primary
  type; pay every newly visible finding and keep the debt baseline at zero.
  Service uptime now reaches its use case through the clock port, and metrics
  recorder identity is carried by a value object (#112).

- Split the embedded facade into definition, execution and participation
  modules while preserving its public methods and behavior (#111).

- Separate ceremony design validation and definition construction into private
  modules while retaining the existing authoring behavior (#107).

- Split ceremony definition collection validation and guard evaluation into
  private modules without changing behavior (#106).

- Split ceremony step configuration parsing into focused private modules,
  preserving defaults and validation for phase 3a work (#105).

- Accept ADR-015 and ADR-016 for phase 3a concurrency and bounded definition
  primitives; record their contracts and the deferred pattern work before
  implementation (#90).

### Fixed

- Reject ceremony starts and runs with missing required context inputs before
  opening an instance or event stream; report every missing name through all
  execution surfaces and allow the same id to be retried with complete input (#113).

- Refuse terminal ceremony transitions while an intervention remains open,
  preserve the journal on refusal, and report that move disabled (#109).

- Ceremony design returns a domain draft; one adapter renders YAML for both
  MCP paths, with serialization DTOs and serde_yaml removed from made-app
  (#104). Rust hosts read DesignedCeremony::definition() and render explicitly.

- Require at least 80% line coverage per production crate, reject reduced
  committed floors, and exercise binary HTTP/gRPC startup and SIGTERM shutdown
  so the server also meets its floor (#103).

- Development marketplace checks accept fetched tags from earlier releases;
  tag builds and publication still require the annotated version tag at HEAD
  (#89).

- The divergences a read-only review of the parity chain (#55–#64) found
  between the two MCP arms, and the places the gate built to catch them could
  not see. Each is a live difference in what a client gets for the same call
  depending on which engine served it, which is what ADR-014 says is a defect.

  **Inside the shared tools.** An omitted `lease_ttl_ms` meant thirty seconds
  in process and sixty over the wire on `made_run_ceremony`; the three
  defaults now belong to the `made-app` inputs that carry them, both arms send
  the number rather than a zero for the server to choose with, and the schemas
  state it. An omitted `idempotency_key` left four different spellings in the
  journal — `made-mcp-`, `made-mcp-external-`, `grpc-`, `grpc-claim-` — where
  the key is sealed as evidence; one rule, `made-mcp:<uuid>`, applied by the
  MCP layer on both arms. A report title was trimmed on one arm and not the
  other, so `" Session review "` rendered two documents; `ReportTitle` carries
  the trim rule and the emptiness rule, and both arms build it.
  `DomainError::Conflict` reached every surface as `refused` with
  `retryable: false`, against what `domain_error_to_status` and
  `AppendOutcome` both say about it; `conflict` is a fifth tool error code and
  a fourth `ApiError`, retryable on both, and the in-process mapper is
  exhaustive where it had a `_` arm. `ReadCeremonyEvents` sampled the head
  before reading the page, so a concurrent append answered `next_version >
  head_version` with `has_more() == false`; the head is read after. A `limit`
  above 1000 was refused by the gate and clamped by the engine — one rule now,
  refused, said the same way in the schema, the engine and the proto comment.
  An explicit `null` on an optional field and a `_meta` inside `arguments`
  were both refused, so a host generated from a typed SDK looked broken; the
  gate reads the first as absent and skips the second. `uniqueItems` uses a
  set and every caller-supplied list of ids declares `maxItems`. The three
  constraints the schema descriptions promised in prose — "exactly one of",
  "at least one seat", "`failed` requires `error`" — are `oneOf`,
  `minProperties` and `if`/`then`/`else`, so a caller's own validator refuses
  what this server refuses.

  **Whole numbers, decided at ingress.** `google.protobuf.Struct` carries
  every number as a double, so `{"score": 1}` and `{"score": 1.0}` are the
  same bytes on the wire and cannot be told apart again — and the sealed
  record of one session depended on which engine wrote it, badly enough that a
  client running `AuditChain::verify` on what it was handed could fail on an
  intact chain. Both arms now read a whole-valued number whole and refuse a
  number outside ±2^53 as `invalid_request`, at the request gate; the gRPC
  server keeps its own ingress for direct clients with the same rule, and
  `docs/architecture/struct-numbers.tsv` is the table a test in each crate is
  pinned against. Carrying the exact bytes on the wire is the alternative and
  is **deferred**: it means a field beside every `Struct` saying which of its
  numbers were written whole, on seven messages, and that is a contract change
  for phase 2.

  **The gate.** The facade scan was a line grep over one file for two
  spellings of `pub fn`, so `pub const fn version` and any second `impl` block
  were invisible; it reads every file under `crates/made-embedded/src` and
  every `pub` form, and every excused method is asserted to be one the scan
  really finds. The surface columns were compared as sets, so two rows with
  swapped cells stayed green; each row's cells are derived from its capability
  key through the transform `protocol/tests.rs` owns, with the genuine
  exceptions listed and reasoned. The parity session ran over
  `InMemoryCeremonyEventStore` on both arms, which is not what the local
  edition ships; it runs a second pass with the in-process arm over WAL-mode
  SQLite. It read one stream's digests, so it reads all three, and its two
  intervention payloads carry a whole number and a `1.0`. The rendered report
  is the serde form of `made-core`'s entities and nothing pinned it, so the
  document the session renders is committed and compared — a rename in the
  domain changes a file in review rather than a document silently.
  `docs/editions.md` says what the gate proves after all of this, including
  the one honest sentence about numbers. (#75)

### Added

- **Durable session memory in the ceremony SQLite store.**
  `SqliteSessionMemory` stores idempotent memory writes through the same
  SQLite engine as ceremony streams and passes the nine-property memory
  conformance suite. `EmbeddedMade::open(path)` and the deployable server's
  path-aware default now preserve decisions across process restarts; the
  generic `EmbeddedMadeBuilder` remains side-effect-free and forgetful unless
  a host supplies memory. `with_ceremony_store_and_memory` gives Rust hosts a
  typed same-adapter entry point. `MADE_MEMORY=none` disables server memory,
  while explicit `sqlite` without `MADE_CEREMONY_STORE_PATH` fails startup.
  A two-process `made-mcp` stdio test proves that a new process recalls the
  earlier process's decision. (#102)
- `CeremonyEventSubscriberPort` in `made-core`: the one seam a projection hangs
  off. It is told the sealed records of one successful append, in order, each
  with its place in the global order, after the store confirmed and before the
  session goes back to the use case — so a caller that reads a projection
  straight after the call it made finds it there. Infallible by signature
  rather than by a swallowed `Result`: a projection cannot fail the session it
  projects, and a subscriber with something to report logs it at warning level
  with the ceremony id and the sequence. `NoopCeremonyEventSubscriber` is the
  default, `CeremonyEventFanout` in `made-app` composes several in a fixed
  order, and `EmbeddedMadeBuilder::with_event_subscriber` adds the host's own
  behind the engine's. Durable consumers are not this port; publication keeps
  a cursor over the global order (A6). Three colocated tests: every record of
  every append observed once and in order across a multi-command session, a
  conflicted-then-retried append observed once with the records that landed,
  and an append that never lands observed not at all (ADR-012). (#66)
- `AppendOutcome::positioned`, and the conformance property
  `an_outcome_positions_what_read_all_returns` that both event-store adapters
  now pass: what an append reports is what a reader finds from that position.
  Telling a subscriber off the outcome is only as good as a read if the two
  agree, and an adapter whose `first_position` was off by one would hand every
  projection a cursor that skips or repeats a record. (#66)
- **`made-mcp migrate-store <path>`**: the one way back into a store written
  before a ceremony was its event stream (ADR-012). Since A4 such a store
  opens, counts its sessions and warns about them, and those sessions are
  invisible and refuse commands; this imports them. Copy-on-write, as ADR-008
  did for redb: the operator's file is never opened for writing, the command
  works on a copy — write-ahead log included — and installs it with two
  renames only after **every** session has been read back out of it and folded
  to exactly the snapshot that went in. A run that did not hold leaves the
  file byte for byte; a run that did keeps the original beside the new store
  as `<path>.pre-stream.backup`, which a later run never overwrites. Running
  it again is a no-op that says so. The legacy `ceremony_instances` and
  `audit_journal` tables stay as read-only provenance behind the new
  `LegacyCeremonySnapshotSourcePort`, and the report of an imported session
  says that what happened before the import was recorded without payloads and
  cannot be recovered. Proved on a store `made-mcp` **v0.3.1** really wrote,
  committed under `crates/made-tests-integration/fixtures/stores/v0.3.1/`
  with the script that produced it: both of its sessions import, each stream
  verifies as a chain, the definition binding and the open intervention
  survive, and the mid-flight session takes its next claim. (#67)
- `CeremonyEvent::InstanceImported`, the genesis event, at schema version 1
  with its golden fixture. It carries the aggregate's own serde shape rather
  than a copy of its fields, because the claim an import makes is fold
  equality with the snapshot and a hand-written field list would make that
  claim only as complete as the list. `apply` replaces the session with it;
  `decide` never emits it; `rehydrate` opens on it and refuses it anywhere
  else, since an import in the middle of a stream would discard everything
  before it. `AuditEventType` gains the matching entry. (#67)
- **`VerifyCeremonyJournal`** on all four surfaces in one change (ADR-014):
  the RPC, `made_verify_ceremony_journal` on both MCP backends,
  `EmbeddedMade::verify_journal`, a `parity.tsv` row with no reason, the
  `ceremony_history` capability group, a step in the F4 session script and
  tool docs. `AuditChain::verify` had existed since ADR-003 with no caller — a
  chain nobody checks is a claim, not evidence — and now has one. The answer
  names the head version, how many records were verified, whether the chain
  holds and, when it does not, the first position that cannot be trusted and
  why, in words written once on `AuditChainDefect`. A broken chain is an
  answer, not an error; a session with no stream is not found. Both MCP arms
  render through one view, so the in-process answer and the one that came back
  over gRPC are the same keys by construction, and an integration test runs
  the verifier itself over the records the wire handed it and compares.
  Additive; `buf breaking` against `origin/main` is green. (#67)
- **Recall at start.** A working session can declare a `memory_scope` in the
  context it is started with, and the engine reads that scope through
  `MemoryReaderPort` before the session opens. What comes back is rendered —
  decisions and constraints first, then observations and outcomes, each group
  in the order memory returned them, bounded at **4096 bytes of summary** with
  a `truncated` flag when the bound bit — and sealed into the stream as a new
  `MemoryRecalled` event, in the same append as `CeremonyInstanceStarted`, so
  a session cannot exist without what it was told. The engine wrote memory and
  never read it: `MemoryReaderPort` had no consumer at all, and the scope was
  always the instance's own id, so no ceremony could ever recall another.
  A scope is now `kind:name` (`ceremony:{id}` is the same grammar), and a
  session that declares none keeps the default, which means **no shared
  memory** and says so in the docs. An entry larger than the whole budget is
  dropped rather than cut. A memory that cannot be read costs a session its
  recollection and never the session: it is logged at `warn` and the start
  succeeds. A `memory_scope` that is present and is not a usable scope refuses
  the start, because falling back to the private default would hand an
  operator a session that remembers alone while they believe it is sharing.
  Gated by `crates/made-tests-integration/tests/mcp_parity_session.rs`, whose
  session now drives two ceremonies in one scope on **both** MCP backends and
  asserts the second carries the first one's decision and shows
  `memory_recalled` at sequence 2 — and asserts that a session declaring no
  scope seals nothing beside its opening, which is why every stream, golden
  and fixture written until now is unchanged (ADR-013, plan §3.8 E1). (#68)
- `recollection` on all four surfaces: `CeremonyRecollectionState` and
  `CeremonyRecalledEntryState` in `underpass.made.v1` with
  `CeremonyInstanceState.recollection` at field 19, the gRPC mapper, both MCP
  presenters, the fixture backend a client wires against, and
  `CeremonySummary.recollection` in `made-api`. A message rather than fields
  on the state, because proto message presence is the one way this contract
  can say "told nothing" without it reading as "told an empty scope"; the MCP
  arms render `null`. `GenerateCeremonyReport` renders a **What earlier
  sessions decided** section ahead of what the session did. Additive;
  `buf breaking` against `origin/main` is green. (#68)
- `authorizes` in the `made_assert_ceremony_reason` schema's `kind` enum. Both
  request mappers accepted it and the schema refused it first, so the one
  relation a reviewer looks for — what made an action allowed — could not be
  asserted through either MCP backend. (#68)

- An **Editions** section in `docs/operations/support-matrix.md`: one row per
  capability group — the groups `made_discover_capabilities` answers with —
  against the four surfaces (the proto contract, MCP on the gRPC backend, MCP
  on the embedded backend, the `EmbeddedMade` facade), each cell `supported` or
  `not supported` with the reason quoted verbatim from
  `docs/architecture/parity.tsv`, plus the gate that proves the row. Parity was
  a checked file and two tests; it was not yet a support claim, and the page
  whose rule is that a claim names its source of truth and its enforcement had
  no edition row at all. A group whose capabilities disagree about a surface is
  split per capability: `ceremony_design` is the one split today, because
  validating and explaining a draft have no facade method by decision (F1).
- `crates/made-mcp/src/protocol/editions_matrix_tests.rs`: the check that keeps
  that table honest. It derives the expected cells from `parity.tsv` and
  `CAPABILITY_GROUPS`, parses the table out of the markdown between two
  HTML-comment markers, and compares them cell by cell in both directions — a
  cell that promises what the TSV denies fails by name, a group the table
  forgets fails, a row the code no longer groups fails, and so does a gate
  column that names a test which does not cover the row. It also asserts that
  every capability with an MCP tool is in exactly one group, and that the two
  capabilities no group can offer (`list_ceremony_definitions`,
  `mount_definition`) are still named in the section. No server, no store, four
  tests in milliseconds inside `cargo test -p made-mcp`, so the development
  loop runs it (ADR-014). (#54)
- `made_get_status` and `made_get_metrics` on the **embedded** MCP backend, and
  the facade methods `EmbeddedMade::status` and `EmbeddedMade::metrics`. The
  two answers are composed by `GetServiceStatusUseCase` and
  `GetServiceMetricsUseCase` in `made-app`, which the deployable edition's
  `GetStatus` / `GetMetrics` handlers now call as well: the version, the
  uptime, the condition and the counters are decided in one place instead of
  being assembled inside a gRPC handler, so the two editions cannot answer
  differently about what they are. Both arms render the four keys the contract
  carries and no fifth. The proto is untouched.
- `MetricsRecorderPort::recorder_name`, and an **in-process Prometheus
  registry as the embedded default**: `EmbeddedMadeBuilder` wires
  `PrometheusMetricsRecorder` when the host wires none, where it used to wire
  a recorder that forgets. No exporter, no endpoint, no new dependency —
  `prometheus` was already in `made-embedded`'s graph through `made-adapters`,
  `Cargo.lock` is unchanged and the embedded dependency boundary still holds.
  A host still injects its own recorder through `with_metrics`, and
  `EmbeddedMade::status` reports which one is running. `EmbeddedMadeBuilder`
  also accepts a statistics port (`with_statistics`), in-memory by default.
  (#53)
- `ReadCeremonyEvents`, `GetCeremonyTranscript` and `GenerateCeremonyReport`
  RPCs in `underpass.made.v1`, the two new tools `made_read_ceremony_events`
  and `made_get_ceremony_transcript` on **both** MCP backends,
  `made_generate_ceremony_report` on the gRPC backend, and the facade methods
  `EmbeddedMade::audit_records_from` and `EmbeddedMade::report`. What a session
  left behind had no read surface at all beyond the in-process facade: the
  event stream that *is* the ceremony (ADR-012) and the transcript its steps
  produced were unreachable from any client, and the report was rendered
  inside the MCP adapter with no RPC behind it. A read of the stream hands out
  the **sealed records** — position, actor, timestamps, correlation and
  causation, the payload the digest covers, and the hash chain — so a client
  reads them back and verifies the chain on what it received rather than
  trusting the server that sent it. Reading is by position: `from_version` is
  the version already seen, and the answer carries `next_version` and
  `head_version`. An omitted `limit` takes 200 records, capped at 1000. A
  durable named cursor per consumer is not this and arrives as additive
  fields. Additive; `buf breaking` against `origin/main` is green. (#52)
- `DesignCeremony` RPC in `underpass.made.v1`, the gRPC-backend tool
  `made_design_ceremony` and `EmbeddedMade::design`. Turning an author's
  intent into a ceremony was a plugin extension with no RPC behind it; it is
  now a use case both editions call, so the same intent renders the same
  document — byte for byte, checked by an integration test that designs on
  both arms — and a host pointed at a cluster can design there. The request
  mirrors the tool's schema field for field, with field presence where absent
  and zero are different answers (`see_prior`, `num_agents`,
  `step_timeout_seconds`, `max_attempts`, `backoff_seconds`). Additive;
  `buf breaking` against `origin/main` is green. `stages[].pattern` is not
  part of it. (#51)
- `ClaimCeremonyStep` and `CompleteCeremonyStep` RPCs in
  `underpass.made.v1`, over the same use cases the embedded edition has always
  called, plus the two gRPC-backend tools `made_claim_ceremony_step` and
  `made_complete_ceremony_step`. The delegated-host protocol — claim the step,
  run it with the host's own agents and tools, report the observable result —
  now works against a cluster with the same calls it takes in process. Both
  answer with the session, like every other move. An absent `idempotency_key`
  or `lease_ttl_ms` takes a server default, and the claim's default lease is
  five minutes because the work it waits on is not the engine's; an absent
  `lease_owner_id` becomes `made-mcp:<backend>` like every other lease this
  MCP server takes. (#50)
- `docs/architecture/parity.tsv`: the checked-in exception list of surface
  gaps (ADR-014), one row per ceremony capability and one column per surface,
  with a reason mandatory on every gap. A gate in `made-mcp` compares it with
  the `underpass.made.v1` RPC list, `GRPC_TOOL_NAMES`, the catalog the
  embedded backend admits, the public methods of `EmbeddedMade` and the
  `CeremonyEngineApi` trait. It fails in **both** directions — a tool, RPC or
  facade method no row names, and a row naming something the code does not
  have — and it also fails on a gap with no reason or a complete row that
  still carries one. It runs with no server, in milliseconds, as part of the
  workspace test job. (#47)
- `CeremonyInstanceState` carries `rehydratable` and `unrehydratable_reason`,
  so `ListCeremonyInstances` can name the one session it could not render.
  Additive; `buf breaking` against `origin/main` is green. (#48)
- `CeremonyEventStorePort` and `CeremonySnapshotStorePort` with conformance
  suites, an in-memory adapter (`InMemoryCeremonyEventStore`) and a SQLite
  adapter on `SqliteCeremonyStore` (tables `ceremony_events`,
  `ceremony_event_log`, `ceremony_snapshots`, `store_meta`); the two-writers
  test also covers the event store. The ceremony use cases run on them:
  load is the fold of the stream, and every mutation appends (#45).
- MADE now ships co-located Codex and Claude Code marketplace catalogs, a
  `made-setup` skill and `/made:setup` command, and checksummed standalone MCP
  executables. A clean marketplace install downloads its release-matched
  engine without requiring Cargo. (#36)
- The domain vocabulary gate (`scripts/ci/domain-vocabulary-boundary.sh`)
  now also refuses other products' vocabulary (`kmp`, `kernel`,
  `rehydration`) in `made-core`, `made-app`, `made-api`, `made-mcp`,
  `made-embedded` and `made-adapters` sources, per ADR-013.

### Changed

- The tree proof accepts far less. `scripts/ci/tree-already-proved.sh` took
  any successful `quality-gate` run whose commit tree matched, with no filter
  on the plan, the event, the head repository or the age. So a docs-only pull
  request — whose run skipped every Rust job and went green — proved the tree
  of the merge commit behind it, and the push to `main` that was meant to run
  the full matrix skipped instead; and because that push was itself a
  successful run, the hole laundered forward. A run now proves its tree only
  when it ran in this repository (`head_repository.full_name` equals
  `GITHUB_REPOSITORY`), is younger than fourteen days, and shows **every** job
  of the full matrix concluded `success` — not skipped, not cancelled. That
  last test is also the plan check, and it is read from the run's job
  conclusions rather than from the plan the run recorded, because on
  `pull_request` the planner runs from the pull request's own head and its
  word for "full" is the pull request's word; the `impact` job records the
  plan — `full`, the gates, the event — in the run summary all the same, for
  the reader. `--self-test` drives the same decision the live path drives,
  with fixtures for the API answers: a partial pull-request proof refused, a
  full one accepted, a push accepted, a fork refused, a stale proof refused,
  a skipped job refused, a cancelled one refused. It runs in the `tree-proof`
  job on every event and in `just workflow-contract`. The skip branch itself
  can only be observed on a push to `main`. (#76)
- `scripts/ci/dev-loop-workflow-contract.py` derives the quality-gate job
  list from the workflow instead of reading a hard-coded table on both sides
  of its own comparison, which is why four drifts were invisible to it: a new
  gate job with no `impact` guard and absent from `gate`'s `needs`, the
  trigger types shrunk to `[ready_for_review]`, its own `--self-test`
  invocation deleted, and `gate` no longer treating `cancelled` as a failure.
  Every job that is not `tree-proof`, `impact` or `gate` must need `impact`
  and be routed by an output the planner's `GATES` tuple actually declares;
  `gate`'s `needs` must equal the workflow's job list exactly, both
  directions; the tree proof's idea of the full matrix must equal it too; and
  all four trigger types are checked on all three workflows that stand down
  on drafts. `quality-gate-plan.py`, `tree-already-proved.sh` and the two
  workflows it had never read joined its sources, and it now fails on any
  `uses:` pinned to a tag rather than a commit. 34 mutation guards, up from
  19. (#76)
- `.github/workflows/plugin-package.yml`: the four-host `package` job runs
  scripts the pull request wrote — the marketplace contract, the plugin
  smoke, the two bootstraps, the packager — and no longer holds
  `contents: write`. The tag-gated release upload is its own job now, waits
  for all four hosts and is the only one that writes.
  `actions/upload-artifact` was unpinned there and again in
  `publish-distribution.yml`, and both are on the commit `dev-loop.yml`
  already used; `dependency-review.yml` dropped its workflow-level
  `pull-requests: write`, which the action needs only to post a summary
  comment this workflow never asks for. The packaging matrix also stops
  waking for prose: its `paths` filter excludes `crates/made-mcp/**/*.md`,
  narrowly, because the markdown under `plugins/made/**` is bundle content
  and still wakes it. (#76)
- The planner no longer exports `cargo_packages`. It was computed from the
  reverse dependency closure, exported, and never read — `clippy`, `test`,
  `coverage` and `benches` are all `--workspace` — and wiring it would have
  been strictly weaker than leaving it out, not equal and not stricter. The
  contract pins `--workspace` on `clippy` and `test` instead, and the closure
  keeps its one honest job: deciding which gates run. (#76)
- The two observability documents now claim only what the code records.
  `docs/made-observability-design.md` lists the twenty-one Prometheus families
  the registry holds and the five legacy series `/metrics` hand-rolls, each row
  naming the file that records it; the alerts and the dashboard read only
  series that exist; the span tree drawn in
  `docs/operations/observability-runbook.md` puts
  `prepare_ceremony_participants` where the code puts it, beside `run_ceremony`
  rather than under it, and the log-message table says which fields each
  message actually carries instead of promising `ceremony_id`, `step_id` and
  `specialty` on all six. What had no code behind it is gone from both:
  gRPC front-door RED, Postgres query latency, deliberation phase durations,
  proposals and revisions histograms, the three validator families, step
  attempts, trace exemplars, provider and judge span attributes, and the
  alerts and panels built on them. What a named slice will land — ceremony
  metrics from the event seam, trace ids, the embedded exporter and registry,
  spans on steps and adapters, the ceremony stream — is in one "Planned — not
  implemented" section per document, each item carrying its slice id, because
  a claim in the present tense about a future capability is the thing
  PRINCIPLES §1 forbids. The runbook also says what the embedded edition has
  had since #53 — an in-process registry, the two tools on both backends, no
  exporter, no endpoint, and honest zeros for the council counters — where it
  used to say there was nothing. `README.md`, `docs/index.md`,
  `docs/editions.md` and `docs/embedded-made.md` follow: the embedded metrics
  default is the Prometheus registry it has been since #53, not
  `NoopMetricsRecorder`, and no surface is described as having embedded-only
  ceremony controls. `docs/orchestration-patterns-plan.md` §0.4 names
  `docs/architecture/parity.tsv` as the count rather than carrying four
  numbers that moved with every slice of WS-F, and the slices that have landed
  — A1–A4, F1–F4, G5, H1–H4, and H5 except its per-crate coverage floors —
  say so in their rows, as ADR-012, ADR-013 and ADR-014 now do in their status
  lines. Four counts that had drifted the other way go with them: the tool
  table in `docs/operations/mcp-stdio.md` had 37 rows under a sentence
  promising 41, and now carries every backend-owned tool in the order
  `GRPC_TOOL_NAMES` lists them; `crates/made-mcp/README.md` said the container
  test checks 35 tools and now names `parity.tsv`; the `justfile` said CI does
  not run coverage, which it has since the impact planner landed; and
  `docs/dev-loop.md` said a green `just check` means a green pull request,
  when `just check` leaves out coverage, the chart and the container image.
  (#69)
- **The transcript is a fold of the event stream, not a store.** Every
  `step_completed` record is one contribution — the event already carries the
  step, the seat the definition gives it and the output the step produced — so
  `GetCeremonyTranscript` projects it on read from
  `CeremonyEventStorePort::read` and nothing is appended beside the append.
  Two consequences, both of them the point: the deployable server kept its
  transcript in process whatever store the state went to and emptied it on
  restart, and it is now exactly as durable as the session; and a step a host
  claimed, performed where the engine cannot see it and reported back never
  reached the transcript at all, because only the two drivers wrote to the
  store. The parity session reads both of its steps where it used to read one.
  A ceremony with no stream is now `NotFound` rather than an empty transcript,
  the way reading its events already was, on both MCP arms. The answer for a
  session the store did serve is byte-identical to the golden captured before
  the store was removed (ADR-012). (#66)
- **What a session leaves behind in memory is a projection of its stream.**
  `SessionMemoryRecorder` implements `CeremonyEventSubscriberPort`; the six use
  cases that held an `Arc<SessionMemoryRecorder>` and called it by hand no
  longer take one, and both composition roots wire it as a subscriber instead.
  The recorder folds the stream up to and including each sealed record, so what
  a projection needs and one record does not carry — the ordinal of a response
  among its item's answers, the ordinal of a reason, whether a move was an
  ending — is derived rather than handed in by a call site. Which writes, which
  keys and which scopes are unchanged, and the scope stays `ceremony:{id}`
  (ADR-012, ADR-013). (#66)
- The development loop iterates on the phase-2 crates — `made-core`,
  `made-app`, `made-adapters`, `made-embedded`, `made-mcp` — in
  `.github/workflows/dev-loop.yml` and `scripts/ci/dev-loop.sh` together;
  `python3 scripts/ci/dev-loop-workflow-contract.py --self-test` is what says
  they did not drift. The parity workstream's set no longer covered where the
  work is, so a draft touching the aggregate got no feedback until it was
  marked ready. (#66)
- `SessionStream::load` folds a stream that opens with an import as well as one
  that opens with a start, so a migrated session loads like any other.
  `docs/embedded-made.md`, `docs/editions.md`, `docs/operations/capability-verification.md`,
  `docs/operations/embedded-ceremony-execution.md`, `charts/made/values.yaml`,
  ADR-006 and three crate READMEs named ports, tables and an in-memory adapter
  that no longer exist; they name the event streams, their global order and the
  folded snapshots instead. (#67)

- `EmbeddedMade::open` wires `InProcessSessionMemory` where it wired a memory
  that forgets, so a declared scope is one an operator can actually use: two
  sessions in one process, in the same scope, and the second is told what the
  first decided. It lives as long as the process and does not pretend
  otherwise — a durable memory in the ceremonies store is plan §3.8 E3. A host
  that hands its own in through `EmbeddedMadeBuilder::with_memory` is
  unaffected, and that method now takes one adapter implementing both memory
  ports: a host that wrote to one backend and read from another would have a
  memory that never recalls what it wrote. (#68)
- `StartCeremonyUseCase` and `StartPublishedCeremonyUseCase` take a
  `MemoryReaderPort`; `CeremonyInstance::decide_start` and `decide_start_bound`
  take the recollection and return the batch of events an opening produces;
  `SessionStream::open` appends that batch. `decide` stays pure — the read
  happens in the use case — and a session with nothing to recall produces the
  single event it always produced. (#68)
- `docs/editions.md` points at the Editions table instead of describing the
  surfaces in prose: the "Surface today" row links it, the sentence that said
  native embedded facades for the council and deliberation APIs are "not
  claimed" is now the pointer, and the parity paragraph names **three** gates
  reading `parity.tsv` rather than two. The stale sentence that still listed
  status and metrics as a gap "the embedded edition gains with G3" is gone —
  they landed in #53, and the council surface is the one gap left.
  `docs/index.md` says what the support matrix now carries. (#54)
- `docs/architecture/parity.tsv`: the `get_status` and `get_metrics` rows name
  all four surfaces and their `G3` reasons are gone. No ceremony **or**
  observability capability is gRPC-only any more; the council surface is the
  only remaining gap. F4's parity session drives both tools on both arms after
  the session it already drove, and its `NORMALISED` list — empty until now —
  carries three entries, all `made_get_status`'s and each with its reason:
  the version and the uptime are the answering engine's own, and the text
  block mirrors them. An entry is keyed by tool as well as path, so a value
  excused for one tool stays compared for every other. (#53)
- `made_get_metrics` on the embedded edition answers with every family the
  deployable edition reports and honest zeros for each: `Statistics` counts
  deliberations and orchestrations, which an edition running no council never
  performs. The ceremony families a session *does* move are in the in-process
  registry; putting that registry on the answer is plan §3.7 G3. (#53)
- Rendering a ceremony report moved out of the MCP adapter into
  `made_app::usecases::GenerateCeremonyReportUseCase`, which composes the
  session, the definition it runs and its event stream into the one document
  both editions answer with (ADR-006: a report is a projection of persisted
  state, never a document the engine stores). The embedded arm is a mapper
  over its DTO and the JSON the tool answers with is unchanged. `persisted`
  stays a constant in the two presenters rather than a field on the wire: no
  edition writes a report, and an always-false boolean in the contract would
  suggest a caller could ask for one that is. (#52)
- `docs/architecture/parity.tsv` no longer carries a gap on
  `read_ceremony_events`, `get_ceremony_transcript` or
  `generate_ceremony_report`, which leaves **no embedded-only ceremony tool**:
  every ceremony capability is served by the proto contract, both MCP backends
  and the facade, and a client pointed at a cluster finds the whole ceremony
  surface. The remaining rows with a reason are the council surface (until B3)
  and the host-process affordances. (#52)
- A whole number a caller sends in a `context`, a step `output`, an
  intervention's `details` or an evidence request is now **stored** whole by
  the deployable server. A `google.protobuf.Struct` carries every number as a
  double, so `2` reached the engine as `2.0` and was sealed into the audit
  record's digest that way, while the same session driven in process sealed
  `2` — one session, two audit chains. Found by comparing the sealed records
  of a parity session, which is the first read that put a digest where a test
  could see it. (#52)
- The ceremony designer moved out of the MCP adapter into
  `made_app::usecases::DesignCeremonyUseCase`, with the intent document as a
  DTO and the definition document it renders private to it. What an omitted
  field means — version, handler, agent count, `see_prior`, timeout,
  attempts, backoff, the approval's guard and trigger — is decided in that one
  place, and an intent that cannot become a ceremony is refused there with a
  reason that names the element at fault. The MCP request type is now a serde
  shape that builds value objects and nothing else; the answer the tool gives
  is unchanged. (#51)
- `DomainError::InvalidDocument` carries the refusal of a document whose parts
  do not fit together, which the `&'static str` variants could not say without
  dropping the caller's own names. Both arms classify it as the caller's to
  fix: `invalid_argument` on the wire, `invalid_request` in process. (#51)
- `docs/architecture/parity.tsv` no longer carries a gap on
  `design_ceremony`, `claim_ceremony_step` or `complete_ceremony_step`: all
  three rows name all four surfaces and their reasons are gone. Agent help now offers the
  delegated-host sequence on the gRPC and fixture backends too, because the
  backend can now serve it. (#50)
- The parity test drives **one session through every shared tool** on both
  MCP backends, with the same step handler, the same evidence source and the
  same frozen clock wired into each, and compares the two answers field for
  field — `output`, `details`, `context` and `evidence_pack` included, and no
  path normalised. Which tools it must cover comes from
  `docs/architecture/parity.tsv`; the hard-coded allowlist of seventeen names
  is gone. (#49)
- A `tools/call` is now validated against the schema the tool publishes,
  once, in the server layer and before any backend is reached. The same call
  is accepted or refused identically whichever engine is mounted, and a
  refusal is the `invalid_request` envelope with one wording rather than
  whichever request mapper looked first. Requests the gRPC arm used to accept
  and ignore — an undeclared field, a value of the wrong type — are refused on
  both arms, as the published schema always said. (#49)
- One value for one thing, whichever backend served the call: every timestamp
  in a ceremony answer is RFC 3339 on both arms (three renderings before, one
  of them `time`'s `Display`); a whole number in a `context`, `details` or
  `output` comes back whole from the gRPC arm instead of as a double; an
  evidence pack is the object on both arms instead of the serialized string
  over the wire; and a run's step status is `completed` on both arms, as the
  session view already said. (#49)
- An omitted `lease_owner_id` on `made_run_ceremony`,
  `made_run_ceremony_step` and `made_claim_ceremony_step` now becomes
  `made-mcp:<backend>` — `made-mcp:embedded` or `made-mcp:grpc` — applied by
  the MCP layer on both backends before the call reaches the engine, and
  stated in the three tool schemas. The server keeps its own default for
  direct gRPC clients; MCP never reaches it. A blank `lease_owner_id` is
  refused on both arms instead of being silently defaulted on one. (#48)
- Every MCP tool failure now carries one structured envelope —
  `{code, message, retryable}` in the result's `structuredContent`, with the
  text content kept — on both backends. `code` is `unavailable`, `not_found`,
  `refused` or `invalid_request`, reusing `made-api`'s `ApiError` vocabulary.
  The `"gRPC {code}: {message}"` prefix and free-text errors are gone, and a
  `tonic::Status` and a `DomainError` that mean the same thing now produce the
  same code. (#48)
- `made_list_ceremony_instances` answers `{count, instances[]}` on both MCP
  backends. Every entry carries `rehydratable` and `reason`; a session whose
  definition the store does not hold is one unreadable entry instead of a
  failed call, on the server as it already was in process. (#48)
- `made_run_ceremony` reports `steps[].iteration` on the embedded backend
  too. The repeat-until change taught the gRPC mapper to emit it and left
  the in-process presenter behind, so one run read two ways. (#48)
- Release publication now validates marketplace parity, waits for the exact
  plugin archive and standalone-binary asset set, and only then fast-forwards
  the stable `marketplace` branch. (#36)
- Audit journal records are now schema version 2 and carry the ceremony
  event — its full payload and its own schema version — inside the sealed
  envelope; the record digest covers the payload, so `AuditChain::verify`
  detects an edited event. Version-1 records from earlier stores still read
  and verify unchanged.
- `CeremonyInstance` now decides commands into events and folds events into
  state (`decide` / `apply` / `rehydrate`); the existing mutators are wrappers
  over that pair, and fold equality — the session is the fold of the events
  its mutations decided — is tested per command and over random sequences.
- Ceremony use cases load the fold of the event stream, decide and append
  with optimistic concurrency: bounded retry (three attempts) for commands
  that commute — seating, step claim and ending, guards, interventions,
  evidence, reasons — and fail-fast for transitions and start. Snapshots are
  a cache written after every append. Every record carries its correlation
  id (the stream's opening) and causation id (the record before it). The
  embedded edition and the server store ceremonies in `ceremony_events`;
  instances from earlier stores that have no stream are counted and warned
  about at open and are not visible until the migration command lands (A7).
- CI now has two modes. While a pull request is a draft, `dev-loop.yml`
  answers it in minutes — `cargo fmt`, clippy and tests for the crates in
  `DEV_PACKAGES`, the architecture, vocabulary and embedded-boundary gates,
  and a `made-mcp` embedded binary for linux-arm64 — and the quality,
  integration and packaging workflows stand down. Marking the pull request
  ready for review wakes all of them on the `ready_for_review` event. A new
  `gate` job fails on purpose on a draft, so a stand-down can never satisfy
  a required check. `just dev` runs `scripts/ci/dev-loop.sh`, the same
  script the workflow runs, and
  `scripts/ci/dev-loop-workflow-contract.py` fails the build if the two
  ever name different crates. The `develop`-branch trigger is gone; that
  branch no longer exists.
- A ready pull request now runs only the gates its change can reach.
  `scripts/ci/quality-gate-plan.py` plans them from the reverse workspace
  dependency closure plus path routing for the independent contracts
  (proto/AsyncAPI, the embedded boundaries, the plugin bundle, the chart,
  the container image, coverage, the publication dry run), and every job in
  `quality-gate.yml` reads its outputs. Unknown paths, the workspace
  manifest, the lockfile, the toolchain, the workflow, the router itself and
  every `workflow_dispatch` run fail closed to the full matrix. A push to
  `main` whose tree was already proved green by the merged pull request's
  gate skips it (`scripts/ci/tree-already-proved.sh`); a merge from an
  out-of-date branch, a conflict resolved in the UI and a direct push still
  run it.

### Removed

- **Breaking, embedding surface.** `CeremonyTranscriptStorePort` and its
  deprecated alias `CeremonyContextStorePort`, `NoopCeremonyTranscriptStore`,
  `InMemoryCeremonyTranscriptStore`, `EmbeddedMadeBuilder::with_transcript_store`
  and `RunCeremonyStepUseCase::with_transcript_store`; the `transcript_store`
  parameter of `RunCeremonyUseCase::new`; the `memory` parameter of
  `ApplyCeremonyTransitionUseCase::new`, `AssertCeremonyReasonUseCase::new`,
  `ApproveCeremonyGuardUseCase::new`, `DeferCeremonyGuardUseCase::new`,
  `CollectCeremonyEvidenceUseCase::new` and
  `RespondToCeremonyInterventionUseCase::new`. A host that supplied a
  transcript store supplies nothing: the transcript is folded from the stream
  it already keeps, and `EmbeddedMade::transcript` and
  `made_get_ceremony_transcript` answer as before. A host that wants a
  projection of its own registers it with
  `EmbeddedMadeBuilder::with_event_subscriber`. `SessionStream::new` takes a
  subscriber as its third argument and `SessionMemoryRecorder::new` takes the
  event store as its second (ADR-012 consequences). The proto contract is
  untouched. (#66)
- The write paths the event stream replaced, none of which had a production
  caller since A4: `CeremonyUnitOfWorkPort`, `CeremonyInstanceRepositoryPort`
  and `AuditJournalPort` whole rather than method by method — what remained of
  them was reading, and reading a pre-stream store is what
  `LegacyCeremonySnapshotSourcePort` does — together with `CeremonyCommit`,
  `CommitOutcome`, `ExpectedRevision`, their SQLite and in-memory adapters and
  their three conformance suites. **This is a breaking change of the embedding
  surface**: a host that compiled against those ports moves to
  `CeremonyEventStorePort` and `CeremonySnapshotStorePort`, as ADR-012 said it
  would, and it ships with a minor version bump. (#67)
- The outbox: `OutboxPort`, `OutboxTransportPort`, the outbox value objects,
  `PublishOutboxUseCase`, the SQLite adapter, the `outbox` table and the
  seven-property conformance suite. Its only writer was the commit above, so
  without it the port, the table, the use case and the suite would all have
  described a capability nothing could exercise. ADR-012 already decided the
  table goes away; §3.1 of the plan re-targets those seven properties at
  cursors, which A6 adds. A store written by an earlier version keeps its
  `outbox` rows on disk — this engine creates the table no longer and never
  drops one. (#67)
- The `state_migrations` table, created since ADR-003 and never written, and
  `CeremonyInstance::migrate_definition_binding` with the pre-rename digest
  scheme and `CeremonyDefinitionDigestMigration` it existed for, superseded by
  ADR-011. The `commits` mode of the `store_writer` test binary goes with them;
  `two_writers_one_store` keeps its `events` test, which proves the same claim
  about the path that is actually taken. (#67)

- `MemoryReaderPort::ask`, `MemoryQuestion`, the `AnsweringQuestions`
  capability and `MemoryDimension`. `ask` had no consumer outside the
  conformance suite, no adapter declared the capability, and the dimension was
  written by the memory projection and read by nobody; the suite now states
  **nine** properties instead of ten, and every method left on the port has a
  production consumer or a property behind it. Nothing is lost by the
  dimension: a contribution is already named after the agenda item it answers
  (`agenda:{item}:contribution:{n}`). A host implementing the port drops its
  `ask` and the `dimension` argument to `MemoryEntry::new` (ADR-013, plan §3.8
  E2). (#68)
- The in-tree KMP memory adapter: the `made-adapters` feature `kmp`, its
  `kmp` module, the CI matrix arm that built it and its live-kernel test.
  Nothing wired it; both composition roots use `ForgetfulMemory`. Memory
  backends other than the in-tree ones are out-of-tree adapters gated by
  the memory conformance suite. The last in-tree revision is `c7dad9f`
  at `crates/made-adapters/src/kmp/` for anyone who needs it.

### Fixed

- Three routing holes let a change reach `main` with no job run at all.
  `docs/architecture/parity.tsv` and `docs/operations/support-matrix.md` are
  `include_str!`'d into `made-mcp` and `made-tests-integration` tests, and the
  ceremony definitions under `tests/e2e/ceremonies/` into `made-e2e-runner`'s
  own sources plus nine more test and unit targets; all of them routed to no
  gate, so a pull request editing only the TSV ran nothing and `gate`
  reported green. They route to `clippy` and `test` now — not `coverage`,
  which would re-run the very tests `test` has already proved. And the rule
  is non-regressable: the planner's self-test walks every `include_str!` /
  `include_bytes!` under `crates/**`, resolves the target relative to the
  source file (and the `concat!(env!("CARGO_MANIFEST_DIR"), …)` form relative
  to the crate), and fails when a target outside its own crate directory
  would route to no Rust job. Thirteen files qualify today. The `tests/e2e/`
  prefix route is gone with it: the kubernetes manifests, the compose file
  and the four Dockerfiles are named one by one, so a new directory there
  fails closed to the full matrix instead of inheriting an empty route from
  its parent. (#76)
- The planner's change boundary dropped deletions and rename sources.
  `git diff --name-only --diff-filter=ACMR` turned `40cb7e5`'s sixteen-file
  diff into three, so a pull request that deleted a crate source and edited a
  document routed to nothing. It reads `git diff -M --name-status` now and
  plans the union of both sides of a rename or a copy, which is the only
  answer that is right whichever side carried the gate. (#76)

## 0.3.0 - 2026-09-03

### Changed

- SQLite is now the sole canonical embedded ceremony store and ships in the
  normal build. `EmbeddedMade::open`, embedded MCP and the deployable ceremony
  path all open SQLite directly; the runtime engine selector is gone. (#32)

### Removed

- Removed the Redb dependency, feature, adapter, constructors, environment
  variables and conversion commands from current binaries. Existing Redb
  stores are refused without modification and must be converted once with the
  verified `made-mcp v0.2.0 share-store` workflow before upgrading. (ADR-011,
  #32)

## 0.2.0 - 2026-09-02

### Added

- Ceremony steps can repeat after a successful result until a top-level
  structured output field equals a declared JSON value. Every repeat requires
  a maximum iteration count; semantic iterations are persisted separately
  from technical retry attempts, projected through gRPC/MCP, and stop with an
  explicit repeat-limit error and outcome metric instead of spinning. (#29)

## 0.1.6 - 2026-08-17

### Added

- `made-mcp share-store [path]` converts a ceremony store to the shared sqlite
  engine in one command, with the three things the manual sequence required
  you to know already handled: it snapshots first (the live store is locked by
  the session asking), it verifies that the converted store holds the same
  ceremonies **before** installing it, and it keeps the original beside the new
  one as `<name>.redb-before-share`. Nothing is ever deleted, and a store
  already on sqlite is a no-op that says so. (#22)

- `made-mcp --version` reports which engines the build carries — `made-mcp
  0.1.5 (engines: redb, sqlite)`. Whether a binary has the sqlite engine
  decides whether two hosts can share a store, and it used to take a failed
  store open to find out. (#23)

### Changed

- A converted store is named for its engine. A SQLite store at
  `ceremonies.redb` works — the engine is read from the file's first bytes,
  never from the name — but anyone listing that directory concludes the
  conversion failed. (#22)

- The plugin launcher refuses to start when both a redb and a sqlite store are
  present at the default location, instead of silently preferring one. Picking
  one means writing ceremonies into a file the operator is not reading. (#22)

- The plugin launcher says so when its files and the binary are different
  versions. They update through different commands and neither announced the
  other, so a stale plugin with a fresh binary kept working by luck — the
  engine updating silently while the launcher and skills stayed old. (#23)

## 0.1.5 - 2026-08-16

### Added

- Two agent hosts can share one ceremony store. The default engine takes one
  process at a time, so an operator running Claude Code and Codex CLI at once
  — both pointed at the same default path by their own plugin registration —
  got no ceremony tools in whichever started second. The opt-in `sqlite`
  feature builds a WAL-mode engine that admits both. `MADE_MCP_ENGINE` picks
  the engine for a new store; an existing store is always opened by the engine
  that wrote it, detected from the file's first bytes. Gated by
  `scripts/ci/embedded-sqlite-gates.sh`, which runs every store contract on
  the new engine and proves two OS processes write one store without losing a
  record. The default build is unchanged and still carries no C engine.
  (ADR-009)

- `made-mcp convert <source> <destination> --engine redb|sqlite` moves an
  existing store between engines, so the feature above reaches a store that
  already has ceremonies in it. Following ADR-008: source read only,
  destination created rather than overwritten, a receipt of what moved. The
  copy moves rows table by table rather than replaying the audit journal — a
  ceremony store is state plus a journal of what happened to it, not a log
  with derived projections.

### Fixed

- The Windows plugin launcher no longer points at `\ceremonies.redb` at the
  drive root. `cmd` expands `%VAR%` for a whole parenthesised block when it
  parses the block, so the state root set inside that block read back as its
  previous — empty — value on the very next line. The launcher is flattened
  with labels, so every read follows its write without depending on delayed
  expansion.

- Both plugin launchers can now reach a store converted to the sqlite engine.
  `MADE_MCP_ENGINE=sqlite` selects `ceremonies.sqlite3` beside the default,
  and a converted store already sitting there is opened without any path being
  set — otherwise a shared store was out of reach of the documented install,
  since each host would have needed an explicit path.

- `MADE_MCP_BIN` lets the plugin launchers run an operator's own binary. The
  release bundle is built without the sqlite engine — which is what keeps the
  default install free of a C toolchain — and the launcher prefers the bundled
  binary over `PATH`, so `cargo install made-mcp --features sqlite` was
  unreachable through the plugin. The variable selects the executable and
  nothing else.

## 0.1.4 - 2026-08-16

### Fixed

- `cargo install made-mcp` now produces a binary that can actually run the
  embedded engine. `default = ["grpc"]` left the in-process backend behind a
  feature that the registry build never enabled, so following the documented
  install and then `MADE_MCP_BACKEND=embedded` got
  `unsupported MADE_MCP_BACKEND value 'embedded'; compiled backends: fixture,
  grpc`. The headline promise of the embedded edition — the real ceremony
  engine with no service behind it — was the one thing the published crate
  could not do. `embedded` is now a default feature; a lean embedded-only
  binary is still `--no-default-features --features embedded`, which is what
  the plugin bundle and the isolation gates build.

- The plugin launcher no longer leaves the host with an MCP server that
  cannot start. It execs `bin/made-mcp` inside the plugin directory, and that
  path is gitignored, so it only exists in a release package — installing
  straight from the repository produced an exit 127 telling the user to
  "build the local plugin bundle", which is not something they can act on.
  Both launchers still prefer the bundled binary, since a release package
  pins the one that plugin version was tested against, and now fall back to
  `made-mcp` on `PATH`. When neither exists the error names both places it
  looked and how to get one. Found by installing the sibling KMP plugin on a
  clean machine and watching it fail the same way.

### Changed

- The README opens with the two editions and the host wiring for Claude Code
  and Codex CLI. New `docs/editions.md` is the canonical embedded-vs-cluster
  comparison, including the three things the embedded surface explicitly does
  not prove; the operations index is grouped by edition.
- Every embedded snippet now sets `MADE_MCP_REDB_PATH`. The backend requires
  it and fail-fasts without it, so the previous example could not start.
- Sibling-repo links point at `kmp` instead of the archived
  `rehydration-kernel`.

## 0.1.3 - 2026-08-15

The release that actually reaches crates.io.

### Added

- Every public crate is published: `made-core`, `made-api`, `made-proto`,
  `made-app`, `made-adapters`, `made-embedded`, `made-mcp-proto` and
  `made-mcp`, in dependency order, by
  `scripts/ci/publish-crates.sh`. The script skips versions already on the
  registry — a release that dies halfway is resumed by re-running the job,
  never by moving a tag — and waits out the new-crate rate limit, which a
  first chain release is guaranteed to hit.
- A README for every published crate. Each states what the crate is, where
  its boundary runs and what it is not allowed to know: `made-adapters`
  that no provider is privileged, `made-embedded` what durability does and
  does not recover, `made-api` why its contract version is not its release
  number.

### Fixed

- `made-mcp` can be published at all. It carries the embedded engine, so
  it requires `made-adapters`, `made-app`, `made-core` and
  `made-embedded`, and cargo resolves every versioned dependency against
  the registry whether or not its feature is enabled. `v0.1.2` failed with
  `no matching package named made-adapters found`; it was the first tag to
  get far enough to say so.

## 0.1.2 - 2026-08-15

### Fixed

- Embedded plugin startup can import the pre-rename Choreographer redb state
  automatically. The legacy database is cloned through a read-only descriptor;
  redb recovery, publication digest migration and instance rebinding happen
  only in a new MADE database and commit with a durable migration receipt.
  Structured startup events expose source SHA-256 and bounded counts without
  logging ceremony content. Existing destinations are never overwritten.
- The two published crates track the release version. `made-mcp` and
  `made-mcp-proto` pinned `0.1.0` literally while every other crate
  inherited the workspace version, so `v0.1.1` would have published crates
  numbered `0.1.0` had it got that far. They now inherit like the rest.
- `just version` moves the internal dependency pins with it. Cargo cannot
  inherit the version that sits next to a path dependency, so a bump left
  every sibling requirement pointing at the previous release — a published
  crate whose dependency does not exist on crates.io.

## 0.1.1 - 2026-08-15

Everything here was found while publishing `v0.1.0`. That tag stays as it
was cut; this is the release that fixes what it shipped.

### Fixed

- Incoming `traceparent` headers are adopted again under the current
  OpenTelemetry bridge. `tracing-opentelemetry` 0.33 refuses to re-parent a
  span whose context has already started — which entering a span now does —
  and it reports that refusal by value, which the adapter was discarding.
  The subscriber turns context activation off so handler spans stay
  re-parentable, and the adapter logs a rejected adoption instead of
  swallowing it. Traces kept exporting throughout; they had quietly stopped
  being the caller's, which is the failure mode worth catching loudly. The
  test now asserts on exported span data rather than on the bridge's
  in-process view, because that view is what changed shape.
- `otel` joins the CI clippy and test matrix, for the adapter and for the
  binary's OTLP exporter setup. The regression above was invisible because
  no CI job ever built that feature; the exporter migration then broke the
  container build, which was the only job that did.
- The OTLP exporter's TLS config is built from the tonic that
  `opentelemetry-otlp` links, one major ahead of the server's. Same name,
  different type: the exporter keeps its own tonic rather than dragging the
  gRPC surface through a migration it does not need.


- The Windows plugin package reaches the GitHub Release. Its attach step is
  a bash script and `windows-latest` runs steps under PowerShell, which read
  the line continuations as unary operators and failed to parse; the step
  now declares `shell: bash`. The bundle itself always built and smoke-tested
  correctly — only publication failed.
- `scripts/plugin/package-made-plugin.sh` empties `dist/plugin` before it
  builds. The release job globs that directory, so a leftover or stray
  archive was published as if it belonged to the version being released.
  The `v0.1.0` release carried one such archive, built from a different
  commit; it has been removed from the release assets.

### Security

- Dependencies refreshed against every advisory open at release time.
  `async-nats` moves to 0.50, which drops the vulnerable `rustls-webpki`
  0.102 line (GHSA-82j2-j2ch-gfr8 and three lower-severity advisories) with
  no source change on our side, and the lockfile refresh takes `quinn-proto`
  to 0.11.16 (GHSA-4w2j-m93h-cj5j), `rand` to 0.8.7 and `serde_with` to
  3.22.0. `testcontainers` moves to 0.28, which replaces the unmaintained
  `tokio-tar` with the patched `astral-tokio-tar` 0.6.4
  (GHSA-j5gw-2vrg-8fgx), and the OpenTelemetry stack moves to 0.32 with
  `tracing-opentelemetry` 0.33 (GHSA-w9wp-h8wv-79jx). No advisory is open
  against this release.

## 0.1.0 - 2026-08-15

First tagged release. Everything below shipped under the previous name,
`underpass-choreographer`, except where an entry says otherwise; the
rename to MADE is itself the first entry under Changed.

### Added

- **Plugin release packaging for Codex and Claude Code.** The bundle now
  carries a Claude Code manifest (`.claude-plugin/plugin.json`) next to the
  Codex one, both stamped from the workspace `Cargo.toml` version by
  `scripts/plugin/package-made-plugin.sh`, which emits
  `dist/plugin/made-plugin-<version>-<os>-<arch>.tar.gz` with a per-archive
  `.sha256` checksum. On a `v*` tag the tag must equal the workspace version
  or packaging fails; the `plugin-package` workflow smoke-tests and packages
  the bundle on linux-x86_64, linux-arm64, macos-arm64 and windows-x86_64 —
  Windows bundles carry `made-mcp.exe` and a `run-embedded-mcp.cmd` launcher
  that defaults its state file under `%LOCALAPPDATA%` — and attaches the
  tarballs to the GitHub Release for tag pushes. The plugin smoke now
  rejects diverging manifest versions.

- **Durable embedded MCP backend.** `MADE_MCP_BACKEND=embedded` now opens the
  redb state file named by `MADE_MCP_REDB_PATH`, so ceremonies started through
  the stdio adapter survive the MCP process. The variable is mandatory —
  without it the binary exits with code 2 instead of inventing a location or
  running on memory that dies with the process — and the Codex plugin launcher
  supplies `${XDG_STATE_HOME:-$HOME/.local/state}/underpass-made/ceremonies.redb`
  by default. `MadeMcpServer::embedded_redb(path)` exposes the same
  composition to Rust hosts; `MadeMcpServer::embedded()` remains the in-memory
  one. The plugin smoke now proves the claim across processes: one launcher
  publishes and starts a ceremony, a second reopens the same file and reads it
  back with its state, next step and bound definition digest intact.
- An embedded ceremony execution runbook covering the delegated-host loop
  (publish → start-published → claim → perform → complete → transition),
  restart recovery, the durability boundary and its failure modes.

- A capability-verification runbook that separates implemented engine features,
  the active MCP executable catalog, execution ownership and configured
  durability. Root, plugin and embedded documentation now direct agents to
  verify each claim independently.
- MCP self-description through `made_discover_capabilities`, derived from
  the active backend-filtered tool catalog, plus `made_get_help` guidance
  for users and agents. Discovery marks artifact generators and their
  persistence boundary; agent help covers preconditions, authority,
  delegated-host sequencing and errors. The Codex plugin smoke now proves the
  report generator is advertised and generates Markdown.
- Embedded MCP adapters for host-owned step execution through
  `made_claim_ceremony_step` and `made_complete_ceremony_step`, reusing the
  existing start/complete application use cases. Guidance distinguishes a
  configured real server handler, the bundled no-op default, and delegated
  host work completed only with observable output/evidence.
- Embedded MCP ceremony reports through `made_generate_ceremony_report`.
  Reports project one or more persisted snapshots, resolved definitions and
  ordered audit journals into deterministic, injection-safe Markdown, return
  structured completion and definition-binding metadata, and perform no write.
- Host-owned MCP server identity for embedded compositions. The default remains
  `underpass-made-mcp`, while an embedding application can advertise its own
  name and version during the MCP initialization handshake.
- Embedded ceremony instance discovery through
  `made_list_ceremony_instances`. Hosts can enumerate recoverable meetings
  after losing conversation context, refresh the selected instance, and resume
  without approving guards, closing interventions, or replaying operational
  work. Process-restart durability remains a responsibility of the repositories
  configured by the embedded host.
- Embedded MCP backend and repo-local Codex plugin bundle. The isolated
  `made-mcp` build completes the MCP stdio handshake and runs ceremonies
  without gRPC/protobuf. Its current backend-filtered catalog also includes
  discovery, authoring, incremental controls, delegated-host execution,
  interventions, evidence and reports; callers must use
  `made_discover_capabilities` for the exact installed surface. Direct,
  process, dependency-boundary and plugin-launcher smoke tests cover the
  bundle.
- `made-embedded`, an in-process distribution of the ceremony engine with
  local defaults, injectable domain ports, an async host-callback step adapter,
  incremental human-active operations, and no required gRPC, NATS or Postgres
  dependency. It uses the same domain and application use cases as the
  deployable binary and carries the same workspace release version.
- Ceremony step `output_contract` (#118): declarative deterministic policy
  gates in ceremony YAML — `contract_id`, `format`, `required_fields`,
  `allowed_values`, optional embedded `json_schema`; unknown keys are
  rejected. Proposals that fail the gate fail the deliberation as
  `NoValidProposal{contract_id}`.
- Evidence grounding rule (#119): optional `evidence` block on an
  `OutputContract` — each claim object must cite `evidence_refs` that exist
  in an allowed set, resolvable per-run from the `RunCeremony` context
  (`allowed_refs_from_context`) or a static list; enforced by the
  `claims-evidence-grounded` validator.
- Helm chart NOTES banner (#115): `helm install` prints a loud warning when
  trace export (OTLP endpoint) is not configured, so deliberations are never
  silently run unobserved.
- Operations runbooks: observability wiring (traces, metrics, logs) (#116)
  and ceremony authoring (schema, rounds, sizing, verification) (#117).
- Observability — Prometheus metrics: the binary exposes the operational
  metric families at `GET /metrics` (HTTP port `8080`) through a
  `MetricsRecorderPort` (core) and a `PrometheusMetricsRecorder` adapter
  (explicit registry, no global recorder), alongside the original
  `Statistics`-backed counters. Covers deliberation quality (duration,
  winner-score distribution, terminal outcome), the LLM judge (latency,
  score, errors by kind, discrimination, tokens, scoring mode), the
  proposing providers (request latency, errors, in-flight gauge, tokens),
  the ceremony engine (outcomes, durations, per-step status, blocked
  transitions), NATS publish (latency + errors), and the Postgres pool.
  Wired through a `with_metrics` opt-in so only the composition root
  installs the live recorder. Covered by unit tests.
- Observability — distributed tracing: with the `otel` feature and an OTLP
  endpoint configured, a deliberation is exported as one trace whose span
  events carry the debate itself — proposals, peer critiques, validator
  verdicts, judge scores, and the winning rationale — over mutual TLS to
  the in-cluster collector.
- Ceremony "meeting record": the winning contribution of each ceremony step
  is returned on the `RunCeremony` response (`CeremonyStepExecution.output`),
  so the full prose outcome of a run is a first-class API artifact.
- LLM-as-judge scoring: an optional `JudgeAwareScoring` strategy fed by an
  `LlmJudgeValidator` that ranks deliberation proposals by intrinsic
  quality instead of validator pass-fraction. Opt-in via
  `MADE_JUDGE_ENABLED` (with `MADE_JUDGE_THRESHOLD`), reusing the vLLM
  endpoint/model; fail-fast wiring and a Helm chart guard refuse a
  judge-on-without-vLLM configuration. Covered by unit tests and a
  provider-backed E2E.
- Ceremony engine: `RunCeremony` executes YAML-defined ceremonies as
  finite-state machines (states, steps with pluggable handlers, guarded
  transitions, roles), with multi-agent panels, a run-time context brief
  injected into each agent's task, and a Mermaid sequence diagram in the
  response. Catalog ceremonies (daily standup, technical debate, sprint
  planning, speaker + Q&A) run end-to-end in CI, driven by the
  `made-run-ceremony` operator tool.
- Helm persistence for the judge + vLLM provider env in the
  `underpass-runtime` overlay, guarded by a CI marker and a chart `fail`
  assertion enforcing the judge↔vLLM coupling.
- Product usability and publication planning:
  `docs/product-usability-publication-plan.md` and
  `docs/product-publication-checklist.md`.
- Explicit documentation that MADE is agnostic and
  independently usable; KMP, PIR, Runtime, and other projects are study
  cases or optional integrations, not required dependencies.
- Local no-external-service quickstart:
  `MADE_NATS_ENABLED=false just run`.
- MCP fixture and live-gRPC quickstarts, plus examples for
  `CreateCouncil`, `RegisterAgent`, `RegisterContract`,
  `RunCouncilDecision`, and `Orchestrate`.
- Repo-owned compose E2E guide covering the compose scenarios, stubs,
  Report schema, and provider-shaped OpenAI/vLLM paths.
- E2E runner scenario selection through `MADE_E2E_SCENARIOS`, with
  groups for `compose`, `cluster-connectivity`, `runtime-stub`, and
  `structured-output`.
- Consumer smoke `positive-path`, including Report contract
  registration, Strict-mode `RunCouncilDecision`, provider-shaped
  OpenAI/vLLM agents, and optional NATS causality assertions.
- Helm install profiles for minimal standalone, embedded NATS,
  Postgres DSN from Secret, provider environment Secret wiring, and the
  Underpass Runtime executor profile.
- Kubernetes deployment guide covering minimal install, embedded NATS,
  gRPC TLS/mTLS, Postgres secret sourcing, provider environment
  secrets, Runtime executor TLS, and operator smokes.
- Support matrix covering Rust toolchain, image tags, chart versions,
  provider adapters, and Kubernetes posture.
- Upgrade, rollback, and operator deploy verification runbooks for
  pinned images, Secret references, OCI chart installs, and smoke
  checks.
- Security policy covering supported scope, private vulnerability
  reporting, coordinated disclosure, deployment hardening, and secret
  containment.

### Changed

- `made_list_ceremony_instances` no longer fails a whole listing because one
  stored instance cannot be rehydrated. An instance whose definition was never
  published — the documented published-definition restart boundary — comes
  back as `{"ceremony_id": …, "rehydratable": false, "reason": …}` beside the
  instances that did recover. Reading that instance by id still fails: the
  listing degrades, the direct read does not pretend.

- **Renamed: Underpass Choreographer is now MADE by Underpass** — the
  Multi-Agent Deliberation Engine. The repository moved to
  `underpass-ai/made`. Every naming surface moved with it, and all of these
  are breaking for existing callers and deployments:
  - crates `choreo-*` → `made-*`, and the server binary `choreo` → `made`;
  - proto package `underpass.choreo.v1` → `underpass.made.v1`, service
    `ChoreographerService` → `MadeService`;
  - MCP tools `choreo_*` → `made_*`;
  - environment variables `CHOREO_*` / `CHOREOGRAPHER_*` → `MADE_*`;
  - NATS subjects `choreo.*` → `made.*` and Prometheus metrics
    `choreo_*` → `made_*`;
  - Helm chart `charts/choreographer` → `charts/made`, default namespace
    `choreographer-system` → `made-system`, image
    `ghcr.io/underpass-ai/underpass-choreographer` →
    `ghcr.io/underpass-ai/made`;
  - Codex plugin `plugins/choreographer` → `plugins/made`.

  Behavior is unchanged: this release renames, it does not re-scope. The
  engine still runs councils, ceremonies, contracts and judge scoring
  exactly as before.

- Contract gate validators tolerate Markdown-fenced JSON payloads (#120):
  a proposal that is *purely* a fenced JSON block is unwrapped before
  validation, so the gate measures evidence quality, not transport
  cosmetics. Mixed prose+fence payloads still fail.

- Kubernetes E2E jobs default to cluster-connectivity scenarios instead
  of running fixture-only stub scenarios against real deployments.
- `make e2e-compose` keeps the full compose group as the fixture-backed
  end-to-end path.
- Helm render checks now cover pinned-image enforcement, TLS secret
  validation, embedded NATS wiring, Runtime executor failure modes,
  Postgres Secret rendering, and provider env Secret rendering.

### Validation

- MCP catalog parity is checked against the gRPC proto surface.
- Compose E2E has been validated through all nine scenarios, including
  structured Report output and provider-shaped paths.
- Kubernetes smoke has been validated with the selected
  cluster-connectivity group.
- `made-consumer-smoke` has been validated for rejection-path and
  positive-path behavior against local MADE, NATS, and
  `made-stub-llm`.

### Security

- Provider credentials, Postgres DSNs, and TLS materials are documented
  as secret-managed inputs, not values-file or descriptor content.
- Chart gates assert hardened pod defaults and prevent accidental
  rendering of literal Postgres DSNs in the Secret-backed profile.

### Known Limits

- No public immutable `v*` tag, release image, OCI chart, or crates.io
  package has been cut yet; current published `sha-*` images are RC
  smoke artifacts, not stable release artifacts.
- `made-mcp` can only be published after `made-mcp-proto v0.1.0`
  is available in crates.io.
- Provider-backed positive smokes are validated with deterministic
  OpenAI-compatible stubs unless a real provider is explicitly wired by
  the operator.
- The Helm chart does not manage Ingress, provider egress allow-lists,
  or multi-replica/state coordination beyond the documented single
  replica posture.
