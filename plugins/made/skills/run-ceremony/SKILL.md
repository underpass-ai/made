---
name: run-ceremony
description: Execute or resume a MADE ceremony when the user asks to run a structured procedure, using real host work and recorded decisions.
---

# Run a ceremony

Discover the running backend/version when uncertain. Read `tools/list` for
exact schemas and `made_get_help` with `audience: "agent"` for available
workflows. For a design request, use `design-ceremony` first.

## Select the execution owner

Use one owner for each step:

- A verified server-owned handler: call `made_run_ceremony_step` and inspect
  its output. One-shot `made_run_ceremony` is suitable only when all ordinary
  handlers are real and no later human decision is required.
- A declared spawn step: call `made_run_ceremony_step` or
  `made_prepare_ceremony_children`. Both claim and execute the spawn through
  the child orchestrator without invoking the ordinary handler. The explicit
  prepare tool accepts the actor plus optional lease owner, idempotency key and
  TTL from its running schema; it does not accept a prior claim fence.
- A delegated host: claim the exact eligible step, retain the accepted claim
  receipt, execute with the host's authorized worker/tools, then complete it
  with the observed status, structured output and artifact/evidence references.

Before delegated work, resolve the host-owned execution profile for that
role/step. Keep requested and actual model/reasoning, required and actual
capabilities, fallback, inheritance, host agent/incarnation and
checkpoint/handoff provenance separate and visible. An unsupported requested
profile is either refused with an actionable reason or resolved through its
explicit fallback. MADE does not switch a live Codex/Claude agent's model; a
checkpoint/handoff is provenance for later work, and the next claim identifies
the new host incarnation. Host agent ids and MADE role ids remain separate,
and an execution profile never widens ceremony authority.

The bundled handler may be `NoopCeremonyStepHandler`. Empty no-op completion
proves wiring only. A claim performs no work and grants no external authority.
Never report simulated, inaccessible or unperformed work as completed evidence.

In builds exposing `claim_fence`, retain the claim's top-level
`structuredContent.claim_fence` before work and return it unchanged on
completion. New completion schemas require it. Never reload a newer claim's
fence to attach an earlier result. A stale refusal changes no accepted work;
refresh the instance and resolve ownership rather than bypassing it.

For work that outlasts its claim TTL, first discover
`made_renew_ceremony_step_lease` (source implementation #202; absent from the
released 0.7.8 catalog). Before expiry, send the original `ceremony_id`,
`step_id`, `claim_fence`, `lease_owner_id`, a unique `renewal_id` and positive
`lease_ttl_ms`. Renew periodically only while the host still owns authorized
work. Retry a lost response with the same id and identical payload; each new
heartbeat needs a new id. Read the receipt and current step's
`effective_lease_expires_at`. Renewal preserves the producer fence and budget
reservation, and never extends absolute deadlines. A replay is the original
receipt, not new authority. Pause permits accepted work to drain, while an
expired/replaced claim, cancellation or authorization loss requires recovery.
Do not report renewal as evidence that an external agent is alive.

## Start and progress

For restart recovery, publish the reviewed definition and start by its
published name/version using the running schema. Starting from supplied YAML
can leave only a snapshot after restart, with `rehydratable: false`.
Keep caller-provided ceremony ids stable and preserve context. Declare the
actual actor id/kind; an agent acting for a person is still `agent`.

After each action, inspect the returned instance. Work only on eligible steps
and apply only enabled transitions. The same step id may recur: compare
`state_visit` when available, `state_iteration`, step `iteration` and technical
`attempt`. A new state iteration differs from a new state visit.
A bounded repeat that exhausts its limit is an explicit failure, not a
completed loop. Hosts schedule concurrent workers; the engine does not spawn
them automatically.

When `claimable_step_ids` contains siblings from a concurrent state, claim a
bounded set with distinct idempotency keys and delegate each accepted receipt
to one host worker or subagent. Keep each returned `claim_fence` paired with
that worker's result, allow completions to arrive out of order, and refresh the
instance after the accepted siblings are complete. An `any` or counted join
does not cancel a sibling that already holds a live lease. Two host processes
may coordinate through the same SQLite store, where durable claims and the
effective `max_parallel` capacity arbitrate ownership.

MCP does not create agents or processes. If the host cannot actually fan out,
drive the eligible steps serially and report that execution honestly. Never
describe a protocol claim as proof that a subagent ran.

Do not claim completion when `isError: true` or `completed: false`.

## Human decisions

When a human guard blocks, present the concrete decision and transition to
the user. Record approval only after their explicit authorization for that
decision in the current conversation. Silence, a prior general instruction
or an agent result does not satisfy a declared human guard. `role_kind: human`
records that authorization; it is not a way to label an agent's own decision.

If the decision is deferred, use `made_defer_ceremony_guard` with the person's
statement, the reason and concrete `reconsider_when` conditions. Leave the
session paused and report its id, state and guard. Deferral does not approve.

### Integrator vocabulary does not widen authority

An `INTEGRATOR` coordinates contributions and may execute only the integration
steps and transitions declared for that role. Keep implementers and an
independent reviewer separate, and keep the human guard on its own approval
transition. Do not infer approval, review completion, or verified host work
from the Integrator's name or from a step being `in_progress`; inspect the
persisted step result and enabled guards. The current runtime records declared
roles, claims, completions and guard decisions. It does not yet provide the
live agent roster/activity or intervention delivery/acknowledgement proposed
by issues #190--#192, so do not report those as runtime facts.

## Recover a session

After context loss, list instances before creating a replacement. Inspect
the matching incomplete session; if several match, let the user identify it.
A non-rehydratable snapshot is evidence of a prior session, not a runnable
session. A successor needs a new identity and explicit recovery provenance.

Reads do not replay side effects or approve decisions. If a claim receipt was
lost, establish which claim performed the work. Never borrow the current
worker's identity. If ownership cannot be recovered, let the lease expire
before a new claim and account for external idempotency.

## Interventions and evidence

Preserve the participant's request in `message`, choose `opinion`,
`investigation` or `action`, and declare the requesting role kind. Empty or
omitted `target_role_ids` addresses the whole table; explicit ids select roles.
Use provenance when selecting an earlier intervention's proposed option.

For a configured read-only source, use `made_collect_ceremony_evidence` with
the exact request and safe selectors. Otherwise obtain the real response with
host capabilities and record it using `made_respond_to_ceremony_intervention`.
Leave an unavailable or empty evidence request open. Close only when the
requesting participant says they are satisfied or asks to close it.

An action intervention does not authorize a consequential mutation or bypass
a human guard. Resolve ambiguous operational investigation to read-only work;
obtain explicit authority for the actual mutation before executing it.

In hardened builds, target ids, report ids and reconsideration conditions
are unique lists with a maximum of 100. Targets may be empty; report ids and
reconsideration conditions must be nonempty. Follow the running schema.

## Inspect and report

Read session events with `made_read_ceremony_events`, passing the returned
`next_version` as the next `from_version`. Use the transcript for ordered
contributions, and journal verification for internal seals/ordering. If the
journal is not intact, report its first invalid position and do not treat
that suffix as verified evidence. Integrity does not establish the truth of
external outputs.

For reports, select exact ceremony ids and use
`made_generate_ceremony_report`. Its `report_markdown` is the artifact and
`persisted: false` leaves saving to the host. Save only to the requested or
otherwise authorized destination and name that destination separately.
