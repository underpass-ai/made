# ADR 019 — Seal the plan, then open an auditable successor

Status: accepted for implementation; acceptance remains open.

Scope: issue #187, C7 lane A. This ADR fixes the contract; the lane implements it.

## Context

A paused ceremony can be resumed under the definition it was started with, or
cancelled. Neither answers the case the issue describes: the definition turned
out to be wrong, work has already been completed under it, and that work is
real. Resuming replays a definition nobody believes in. Cancelling and starting
again strands sealed evidence and the external effects behind it, and gives no
record of why a second ceremony exists.

The predecessor's stream is authoritative. Definition identity is part of
restart recovery, so the definition behind an existing stream cannot be swapped
in place. Outstanding claims may still hold live leases over external effects.
Any handoff has to answer three questions durably: what was carried, what
happened to each outstanding claim, and which instance succeeds which.

## Decision

Succession is sealed in the predecessor first and opened in the successor
second. `SuccessorPlanned` carries the whole plan — plan identity, the
deterministic successor id, both definition pins, the carried evidence map, one
disposition per outstanding claim and the budget disposition. Only after that
fact is appended is the successor's stream opened, with an expected-empty append
and, on `AlreadyExists`, an exact comparison of the existing opening against the
expected batch. That is the `open_or_verify_child` template already used for
child ceremonies; a stream that exists with different content is a foreign
ceremony and is refused.

A crash between the two steps is resumable, not duplicated. Re-sealing an
identical plan is a no-op; re-sealing a different plan under the same identity
conflicts; re-opening verifies instead of appending.

A predecessor with a planned successor cannot resume. Lifecycle refuses with
`superseded_by_successor`. Cancellation remains available, because abandoning a
superseded origin is still a legitimate decision.

Carried evidence is addressed by `SourceRecordRef`: ceremony, step, event id,
record hash, state visit and attempt. It is a reference between instances.
`CeremonyRecordRef` addresses a record inside its own instance and is not
reused for this. A carried step record keeps `carried_from`, so a step that was
never executed in the successor is never presented as if it had been. Evidence
is referenced, not copied: the predecessor's stream remains the place the work
actually happened.

Every outstanding claim needs an explicit disposition whose fence matches the
one the plan observed: abandoned with no external effect, abandoned with a
reconciled effect and its evidence, carried as a receipt, or retried in the
successor. There is no implicit abandonment, and a stale fence fails the plan
rather than being ignored.

A step can only carry evidence when the definition diff says the successor
carries that step. A step the diff strands cannot carry, and a successor step
that does not exist fails the plan by id.

The successor seals its own deadlines from its own definition. Budget is
declared `Fresh`. Planning is a separate read-only capability that reports
blockers, the diff, the preflight and the dispositions it would require, so the
decision is taken with the answer in hand rather than by trial.

## Alternatives rejected

Editing the paused instance's definition in place. Definition identity is part
of every sealed record and of restart recovery; rewriting it makes the existing
stream unreadable against its own pin and destroys the audit value of the work
already done.

Opening the successor first and sealing in the origin afterwards. A crash
between the two leaves a ceremony with no recorded provenance, indistinguishable
from an unrelated instance that happens to hold that id.

Copying the predecessor's events into the successor's stream. That duplicates
the audit chain, breaks hash continuity, and presents evidence produced under
one definition as if it had been produced under another.

Reusing `CeremonyRecordRef`. It cannot name the instance, which is the one thing
a succession reference has to do.

Transferring the remaining budget. Reservations, root accounting and refunds are
held per ceremony; moving a remaining balance without the reservation ledger
following it would leave two ceremonies believing they hold the same balance.

Introducing `CeremonyFailed` as the predecessor's terminal fact. The event is
catalogued without a producer and is not introduced in this cut (ADR 022).

## Consequences

Two sealed facts, `SuccessorPlanned` and `SuccessionCarried`, and two
capabilities across the four surfaces. `CeremonyInstanceStarted` gains an
optional succession and the step execution record an optional `carried_from`;
both are skipped when absent, so existing V1 goldens keep their bytes.

The instance view exposes the relation in both directions, so "which ceremony
replaced this one" and "where did this evidence come from" are answerable
without reading raw streams.

Authorization is checked on the predecessor and, separately, on the target
definition. Planning a successor does not grant the right to start one.

## Declared limits of this version

`TransferRemaining` is refused with an explanation; budget always starts fresh.
Only completed step outputs are carried; artifacts and receipts are referenced
through their dispositions, never duplicated. A predecessor that is not paused
cannot hand off.
