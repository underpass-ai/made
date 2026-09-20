# ADR 022 — The integrator loop runs on derived attention

Status: accepted for implementation; acceptance remains open.

Scope: issue #204, C7 lane D. This ADR fixes the contract; the lane implements
it.

## Context

Today every round trip stops with the user. An integrator working inside a host
task learns that a step finished, a review was rejected or a human decision is
pending only because a person goes and looks. The issue asks for the loop to
close: MADE pushes what needs attention to the integrator's host, the integrator
acts with the capabilities it already has, records what it did, and comes back
for the next item — stopping deliberately when a human is required, when the
work ends, or when it stops making progress.

Two things must not happen on the way. Delivering an event must not grant a
permission. And a loop that cannot make progress must not keep circling.

## Decision

Attention is a typed projection of the global feed per binding, consumed by a
durable cursor, not a new family of sealed events. A completed step yields a
result available; a failed step or an exceeded step deadline yields a step
failure; a transition into a state whose guard is an unapproved human approval
yields a human decision request, once per guard and visit; an intervention whose
target accepts the integrator's role yields an intervention request; completion,
cancellation and ceremony or state deadlines yield an ending. A pause yields
nothing — it is a state of the loop, not something to wake anyone for.
Unsealed sources are admitted but marked as such: a host reporting its execution
finished while the step is still in progress is a result pending registration,
a host reporting blocked is blocked, and silence for longer than the configured
inactivity window is a coalesced inactivity signal.

Each attention event becomes a delivery in the shared ledger of ADR 020, aimed
at the integrator binding, deduplicated by its stable identity.

Loop state is derived, not stored: executing while claims are live, awaiting
results when nothing is claimable and deliveries are outstanding, awaiting a
human decision when a guard is waiting, paused with the lifecycle, completed or
failed at a terminal, and blocked when something blocked is unprocessed or the
no-progress bound is reached.

Host activation has two adapters. With `none`, discovery declares activation
unsupported and the host follows through a bounded long poll, capped like the
existing ceremony stream. With `command`, MADE runs exactly the command the
operator configured in the environment, with the JSON envelope on stdin, the
binding's destination and host kind passed as environment variables, a cleared
environment otherwise, and bounded time and output. Nothing from the payload is
ever executed, and nothing from the payload builds the command line.

An exit code of zero is a transport acknowledgement. It says the envelope was
accepted, not that anything was done with it. Processing is a separate, explicit
acknowledgement from the integrator.

The integrator records intent before effect. It acknowledges the delivery with
its intended action, keeps the lease, performs the effect through the existing
capabilities under their own authorization, then records the processed marker
with the cursor it advanced. Retrying with the same intent identity is a no-op.
An attention event confers no authority: every effect is still a claim, a
completion, a transition or an approval, authorized as it already is.

The binding fence guards the loop. An acknowledgement or a wait carrying a stale
incarnation or fence is refused with no effect, so a replaced host cannot act on
behalf of the current one. The batch carries minimal context only; the
integrator revalidates the instance before acting, and the contract says so.

Backpressure is bounded per binding. On overflow the oldest non-blocking item is
dropped — a result available, an inactivity signal — and a blocked delivery
records the overflow. A human decision request, a blocked signal and an ending
are never dropped. Progress bounds stop the loop rather than letting it spin:
consecutive rounds with an unchanged journal head and no new processing mark it
blocked once, and a round limit stops queueing results while still delivering
human decisions and endings. The documented sequence tells the integrator to
stop and talk to the user at completed, failed, blocked and awaiting a human
decision.

A rejected review is derived, not invented: a completed step whose output
records that it was not accepted, by documented convention, or whose output
field guard value only enables transitions back into states already visited.
`CeremonyFailed` stays catalogued without a producer and is not introduced here.

Acceptance with a real host is proved by hand with Claude Code and Codex and
recorded as evidence. A host that does not accept activation is written down as
a limit and keeps `none` in discovery. Nothing is simulated to claim the loop
works.

## Alternatives rejected

New sealed attention events. Attention is a reading of facts that are already
sealed, for one audience. Sealing it would add events whose only producer is a
projection, make a ceremony's stream depend on who happens to be watching, and
create a second truth the moment a projection rule changes.

Introducing `CeremonyFailed` for a rejected review. The event has no producer
today; premiering it here would make an ordinary rejection terminal, when the
same signal is already derivable from the step output and the guard.

Storing loop state. It is a function of claims, deliveries and lifecycle;
storing it adds a fourth thing to keep consistent with three sources that
already disagree at different moments.

Activating a command carried in the payload, or a destination-supplied
executable. MADE would be running what a caller supplied. Only the operator's
environment decides what runs; the destination is data handed to that command.

Treating the activation exit code, or a host-reported label, as proof of
processing. Both are claims by the transport or the reporter. The processed
marker is the only thing that says the integrator acted.

Polling as the only mechanism. A host that can be woken should not have to poll,
and a host that cannot is already served by the bounded wait.

Exactly-once delivery. Delivery is at-least-once with a deterministic identity
and a durable processed marker, because the host is outside MADE's control.

## Consequences

Five capabilities across the four surfaces in a new capability group, a durable
feed consumer per binding, and discovery and help that state the activation
adapter and the integrator's sequence explicitly. Instantiating an agentic
system creates the binding with that system's attention policy; without a
system, a binding takes an explicit policy or the defaults.

The acceptance ceremony is part of the deliverable: a delegate, implement,
review, human approval procedure where the user supplies only the initial intent
and the approval, a first review is rejected and corrected, a human decision is
communicated without being taken by the loop, and the run survives a restart, a
pause and a stall. No rejected review may advance as accepted and no approval
may be produced by the loop, asserted explicitly.

Restart is a tested property, not an assumption: killing the process between
queueing and activation, between activation and intent, and between the effect
and its processing marker, then reopening the store, must recover the pending
item without repeating the effect and without a second integrator.

## Declared limits of this version

Host activation is not a verified capability until the manual evidence for a
given host exists; until then discovery declares `none` for that host and the
bounded wait is the supported path. The context delivered with an attention item
is minimal and not authoritative; the integrator revalidates before acting.
Bindings follow the authorization scopes already available, which for a system
execution means the global scope (ADR 021).
