# ADR-012: A ceremony is its event stream

Status: Implemented (decided 2026-09-16; verified 2026-09-18). A1–A4 landed in
#42–#45, A5 in #83, A6 in #108, A7 in #86, and A8 in #121. The slices are §3.1 of
[`../orchestration-patterns-plan.md`](../orchestration-patterns-plan.md)

Supersedes one sentence of ADR-003 ("Snapshot plus append-only journal plus
outbox, not event sourcing") and the description in ADR-009 of a store as
"state plus a journal of what happened to it". Everything else in ADR-003
stands.

## Context

ADR-003 chose a snapshot as the source of truth, a hash-chained journal of
receipts beside it, and an outbox for publication. The code matches that
decision exactly, and the code shows the cost:

- The journal records *that* something happened, never *what*. `AuditRecord`
  has no payload, so ADR-006 reports render every content section from the
  snapshot and use the journal only for a timeline.
- Nothing can be rebuilt. There is no `apply`, no rehydration, no replay; the
  snapshot is written whole on every commit and the journal cannot verify it.
- The outbox is built, conformance-tested and unused: no production commit
  enqueues a message, no transport exists, no ceremony event has ever been
  published.
- Two writers cannot both land. A conflict is detected correctly and then
  returned to the caller; no use case retries, because reapplying a command
  to a snapshot means redoing the mutation by hand.
- Ceremony observability had to be hand-placed: metrics exist in one use
  case out of thirteen, in one driver out of three, and only in the cluster
  edition.

The next capabilities — concurrent steps, ceremonies that broadcast to other
ceremonies, observability in the embedded edition, memory as a projection —
all need the same thing: a complete, ordered, replayable record of what a
ceremony did, from which everything else can be derived.

## Decision

**The event stream is the source of truth for a ceremony.** A
`CeremonyInstance` is the fold of its events. Every command is decided
against that fold and yields events; the events are appended; the state is
never written.

**The aggregate decides and applies, separately.** `decide(command,
&definition)` holds every invariant, lease, idempotency and authorization
rule and returns events or a `DomainError`. `apply(&event)` mutates state
and cannot fail. `rehydrate(definition, events)` is the fold. The history
lists the snapshot carried (transitions, guard decisions, reasons, iteration
records) are derived state.

**One stream per ceremony, sealed as today.** Events are keyed by
`(ceremony_id, sequence)`, contiguous from 1. The envelope is the existing
`AuditRecord` with the event inside; the digest covers the payload;
`AuditChain::verify` verifies content, not only order. Event ids stay
derived and stable; the store rejects a duplicate id within a stream, which
replaces the unbounded idempotency-key set in the snapshot.

**Optimistic concurrency by stream version.** `append(stream,
expected_version, events)` writes nothing on a stale expectation and returns
a conflict. Commands that commute — completing two different steps of a
concurrent state — are retried by reloading the fold, deciding again and
appending, a bounded number of times. Commands that do not commute — a
transition, a start — fail fast.

**Snapshots are a cache.** They are optional, keyed by `(ceremony_id,
version)`, and deleting all of them changes speed and nothing else. A
conformance property asserts that every stored snapshot equals the fold of
the events up to its version.

**Everything else is a projection.** The instance view, the transcript, the
report, memory, metrics, span events, log lines and published events are
consumers of the stream through one subscriber port, each with its own
durable cursor when it needs one. The transcript store port and the outbox
table go away: the transcript is folded from `StepCompleted`, and
publication is a cursor over the stream — at-least-once, ordered per
ceremony, idempotent by event id.

Automatic publishers retry a failed pending position in the awaited append or
startup path, with bounded backoff, until the durable cursor acknowledges or
quarantines it. One subscriber notification drains at most one bounded page;
startup recovery drains finite pages before serving. Explicit pull keeps its
caller-driven acknowledgement contract. For core NATS, adapter success means
that `async-nats` accepted the publish command and drained its client buffer to
the transport within the adapter deadline. It is not a broker, subscriber or
JetStream acknowledgement and does not prove remote replay durability.

**The engine still owns the contract; the host still owns durability.**
`CeremonyEventStorePort` and `CeremonySnapshotStorePort` replace the unit of
work and the instance repository's write path. The conformance suites are
part of the contract: append-only, contiguous, conflict on stale version,
duplicate id rejected, one winner under concurrent appends, chain intact,
fold equality. SQLite remains the reference implementation behind the
existing seam, one table for events with a global position for cursors, one
for snapshots.

**Existing stores migrate copy-on-write.** Their events cannot be recovered;
for each instance one genesis event `InstanceImported` carrying the snapshot
and the legacy chain head opens the stream. The legacy tables stay
read-only as provenance, the report keeps rendering their timeline, and the
imported instances say that they have no per-event payload before the
import. `made-mcp migrate-store` does it once, verifies fold equality
against the imported snapshot before installing, and keeps the original
beside the new file.

**Scope.** Ceremonies. The council `Deliberation` keeps its snapshot and its
five bus events until ceremonies are done; it is the next candidate, not
part of this decision.

## Implementation result

Ceremony commands now append sealed events and rebuild instances by folding
their streams; optional snapshots only shorten the tail read. Transcript and
memory projections consume the sealed records (#83), the legacy write paths
and outbox were removed with copy-on-write migration (#86), and durable
consumer cursors drive pull, NATS publication and the embedded JSONL sink
(#108). Metrics, traces, structured logs and bounded reports project the same
records in both editions (#110), while embedded service metrics, OTLP wiring
and JSONL registry snapshots are composed through host-owned adapters (#119).
Automatic NATS and JSONL publishers now retry a transient pending position
without waiting for a later stream append, and core NATS drains its client
buffer before advancing the cursor (#124, #125).

## Consequences

- A report answers "what was said, decided and produced" from the stream
  alone; ADR-006 is fulfilled instead of limited.
- Two hosts sharing a store can drive a concurrent state without failing
  each other; the retry is a reload of events, not a hand-written merge.
- Observability, memory and publication become adapters of one port, so the
  embedded edition observes exactly what the cluster observes.
- Every event type has a schema version and an upcaster; adding a field is
  a new version with a reader for the old one, never an in-place edit.
- Storage grows with history and the snapshot cadence is a tuning knob, not
  a correctness one. The cost is measured under `docs/experiments/` before a
  default is chosen.
- Deterministic replay is still not promised over non-deterministic
  components. What replays is the ceremony's own history; what a provider
  said is recorded, not recomputed.
- Hosts that compiled against `CeremonyUnitOfWorkPort` or
  `CeremonyTranscriptStorePort` migrate to the event store port; this is a
  breaking change of the embedding surface and ships with a minor version
  bump and a migration note.
