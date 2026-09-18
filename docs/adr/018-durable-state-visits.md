# ADR-018: Durable state visits

Status: Accepted (2026-09-18)

## Decision

A state visit is a typed positive ordinal, separate from state iteration, step
iteration and retry attempt. The initial state is visit 1. Each new transition
opens the next visit, including self transitions and entry into a terminal state.
Within-state repetition keeps the visit and consumes no transition budget.

New `TransitionApplied` payloads seal a destination entry: the new visit and
exact destination step IDs. Applying this entry archives each executed destination
record, preserving all coordinates and results, then replaces every listed record
with a pending record at the new visit, state iteration 1, step iteration 1 and
attempt 1. Unexecuted pending records need no archive. Existing history is retained.
The fold needs no definition and does not infer the reset from current metadata.
The reset concerns destination step records. Context, guard decisions, participant
bindings and interventions retain their ceremony scope; a visit does not revoke
an approval or erase a context write. Pending placeholders for states not yet
entered carry the default coordinate until the entry payload initializes them.

Step, context-write and state-repeat events seal their visit. Fact identities
include it so the same step on a later visit cannot collide with an earlier fact.
Views, execution traces and transcripts carry the coordinate on all surfaces.

## Historical compatibility

An absent snapshot or record coordinate means visit 1 and serializes without a
new field. An old transition without an entry payload keeps the old fold: it
moves the state and resets the ambient state iteration without resetting records.
Old event payloads retain their presence-backed fields and schema versions, so
round trips preserve their sealed bytes and hashes. Old streams are not rewritten.
The first new transition after a legacy prefix opens visit 2; subsequent new
transitions increment that ordinal. Legacy history remains visit 1 because no
historical durable visits were recorded.

Only events with new visit payloads receive new schema versions. Readers reject
mismatched versions and contradictory destination payloads. Snapshots remain a
cache of the fold, including across the legacy/new boundary.

| Event | Historical versions retained | Visit-bearing version |
| --- | --- | --- |
| `StepStarted` | 1, 2, 3 | 4 |
| `StepCompleted`, `StepFailed` | 1, 2 | 3 |
| `TransitionApplied` | 1, 2 | 3, with source visit and destination reset |
| `ContextWritten`, `StateIterationStarted` | 1 | 2 |

The initial opening payload is unchanged: its pending records begin at visit 1.
An absent visit in a historical event remains absent when reserialized.

## Consequences

Bounded cyclic ceremonies rerun destination work. Archives and transcript entries
retain earlier successful work, while guards evaluate the current records. Host
completion fencing (#127) must include the visit so stale work cannot finish a
later claim.
Transition caps still count only actual transitions. A visit identifies an entry
within this ceremony; it is not a per-state counter or a repetition policy.

## Verification

- `made-core/tests/state_visit_compatibility.rs` pins old/new payload versions
  and refuses absent, contradictory, duplicate and zero destination coordinates.
- `made-core/tests/transition_budgets.rs` contrasts the new reentry with the old
  transition fold and checks that caps derive only from actual moves.
- `made-embedded/tests/state_visits.rs` drives real claimed work and context
  writes across A → B → A, repeats within both A visits, checks unique fact IDs,
  restarts between two writer processes and compares every snapshot cut to replay.
- `made-tests-integration/tests/mcp_parity_session/state_visits.rs` compares both
  MCP editions field for field and reads the same coordinates through direct
  gRPC. The Rust facade is exercised by the process-restart test.
