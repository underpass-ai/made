# ADR 020 — One shared host delivery core

Status: accepted for implementation; acceptance remains open.

Scope: issue #192 and the shared foundation it lands on, C7 lane K. This ADR
fixes the contract; the lanes implement it.

## Context

Two issues in this cut ask for the same thing in different words. #192 has to
put an intervention in front of a live agent and know whether it arrived. #204
has to put an attention event in front of an integrator's host task and know
whether it was acted on. Both need durable queueing to a host destination,
observable states, an exclusive lease, an acknowledgement carrying an
observation, deduplication by a stable identity, expiry, and a way to follow a
destination that was replaced.

Built twice, that becomes two vocabularies, two retry policies, two dedup keys
and two different answers to "did this arrive". The existing surfaces also make
it easy to claim delivery without evidence: a participant can report a label
saying an intervention was delivered, and nothing behind that label is verified.

## Decision

One durable host delivery ledger serves both issues. A record is identified
deterministically by ceremony, item kind, item id and target key, so enqueueing
the same item for the same target twice yields the existing record rather than a
second delivery. Its state is one of queued, leased, delivered to host,
acknowledged, processed, failed, expired or superseded, with a bounded history
of transitions. Attempts are counted; a policy bounds them and exhaustion is
visible rather than silent.

The template is `CeremonyEventCursorPort`: an expiring exclusive lease, a fence
that stops a replaced worker from committing, counted failures, visible
exhaustion, and a storage-independent conformance suite every adapter has to
pass — memory, SQLite with reopening, and PostgreSQL.

Acknowledgement is per item and idempotent. The same lease with the same
observation returns the existing acknowledgement; the same delivery with a
different observation conflicts; a foreign or expired lease is refused.
Processing is a separate, later marker, reachable only from acknowledged, and
repeating it with the same action is a no-op. A refusal or an incapacity is a
recorded observation with its own kind, not a silent retry.

Transport facts do not enter the sealed journal. Queueing, leasing, delivery to
a host, expiry and failure live in the ledger. Exactly one domain fact reaches
the journal for #192: `InterventionDeliveryAcknowledged`, which records that a
named recipient observed a named delivery at a named time. The attention events
of #204 are a typed projection of the global feed (ADR 022), not new sealed
events.

An integrator binding names the host destination for a ceremony or a system
execution and carries a monotonic fence. Replacing a binding raises the fence;
a holder of an old fence is refused with no effect, so two hosts cannot both
believe they are the current integrator. Binding with replacement disabled
against a different live binding is refused rather than silently winning.

Supersession is explicit. An exact target replaced by another supersedes the
pending record without re-delivering it. A role target with replacement
following enabled produces a new record linked to the superseded one, so the
chain stays readable.

`HostActivationPort` is the attempt to wake a host. It returns accepted with a
transport receipt, unsupported, or failed. The `none` adapter lands here and
declares activation unsupported. The envelope a host receives carries
identifiers, kind, reason and evidence references — no secrets and no
reasoning. It is the whole of what the host sees.

This lane adds no capability. Its builders are composition, not public surface,
and are recorded as non-capabilities.

## Alternatives rejected

The council journal as the template. Its acknowledgement atomically advances a
positional cursor and releases the lease. That is progress over an ordered feed,
not an outcome recorded per item: a repeated acknowledgement of the same item
cannot return "already acknowledged", a refusal has nowhere to live, and there
is no attempt count to exhaust.

Sealing transport facts in the ceremony journal. The journal is the authority
for what a ceremony decided. A queue attempt is neither a decision nor durable
truth about the world. Sealing attempts would grow every stream with retries,
move event goldens, make a transport retry replay as a domain event, and — worst
— make "delivered" auditable as a ceremony fact when nobody has observed
anything.

A separate mechanism per issue. Two dedup keys and two retry policies produce
two incompatible answers to the only question either issue asks.

An in-memory queue with best-effort delivery. A restart loses pending work, and
the operator cannot tell "never delivered" from "delivered and ignored".

Treating activation success as processing. An accepted activation means the
transport took the envelope. Treating it as processing would write "handled"
into the record with no observation behind it.

Keeping the host-reported delivery labels as evidence. They remain available as
host-reported status, but they are a claim by the reporter. Guidance says so
explicitly: the evidence of delivery is the sealed acknowledgement.

## Consequences

A new table family with memory, SQLite and PostgreSQL adapters and conformance
suites covering the properties above, including survival of reopening. A
delivery status projection computed from journal and ledger together, which can
never report delivered without a real lease or acknowledgement behind it.

Interventions gain a target that names a specific agent execution and
incarnation, an intent, a delivery policy, a supervisor route and the recorded
acknowledgements. All of it is additive and skipped when absent, so existing V1
goldens keep their bytes.

A supervisor may ask without holding the participant role, when ambient
authorization evidence allows it. Authority to ask is not authority to mutate:
the supervisor gains no other action, and closing an intervention remains the
requester's.

## Declared limits of this version

Delivery is at-least-once with deduplication by a stable identity; MADE does not
promise exactly-once to a host it does not control. The `command` activation
adapter is not part of this lane; with `none` composed, discovery declares host
activation unsupported and offers bounded follow instead. Acknowledgement proves
that a host received an item, not that the host understood or acted on it —
that is the separate processing marker.
