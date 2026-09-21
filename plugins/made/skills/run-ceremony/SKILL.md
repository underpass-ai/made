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

For live delegated visibility, report bounded status against the accepted
claim. `running`, `waiting`, `blocked` and `finished` describe execution;
`fresh`, `stale`, `unreachable` and `unknown` describe host observation.
List/get reads are authorized and paginated. Reports carry logical worker,
host agent/incarnation, sequence, idempotency key and claim fence. A handoff
keeps previous executor provenance; missing reports become stale/unknown,
never invented completion or failure. Hosts that cannot discover runtime
status advertise that limitation explicitly.

Use the bounded public `activity` labels `checkpoint_available`,
`intervention_delivered`, `intervention_answered`, `input_requested`, `failed`
and `heartbeat` when that semantic applies. Adding or clearing `blocker`
produces blocker-opened or blocker-resolved activity; a changed host incarnation
with valid predecessor provenance produces replacement. Never put logs, token
deltas, secrets or private reasoning in status fields.

Follow public activity with `made_stream_ceremony` and
`include_agent_activity: true`. A fresh read (`after_activity_sequence: 0`)
returns the filtered current roster snapshot before subsequent activity.
Persist both `resume_after_sequence` and `resume_after_activity_sequence`, then
send both back unchanged on reconnect with the same `role_id`, `step_id` or
`agent_execution_id` filters. Activity sequences remain global, so filtered
results may skip numbers; the returned cursor already advances across those
hidden records. A retention-expired cursor is refused instead of silently
skipping history. `host_assertion` is host evidence, `engine_transition` is a
sealed ceremony fact, and `accepted_result` is an engine-accepted outcome.
Never treat the first as either of the latter two.

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
roles, claims, completions and guard decisions, the live agent roster, and
intervention delivery with acknowledgement. Report a delivery only from the
projected status and the sealed acknowledgement, never from a participant's
own activity label.

## Recover a session

After context loss, list instances before creating a replacement. Inspect
the matching incomplete session; if several match, let the user identify it.
A non-rehydratable snapshot is evidence of a prior session, not a runnable
session. A successor needs a new identity and explicit recovery provenance.

Reads do not replay side effects or approve decisions. If a claim receipt was
lost, establish which claim performed the work. Never borrow the current
worker's identity. If ownership cannot be recovered, let the lease expire
before a new claim and account for external idempotency.

For a planned host handoff, pause only closes new MADE admission; it does not
stop or diagnose the host worker. Inspect `made_inspect_ceremony_resume` and
follow `next_after_claim` to the end before reporting coordination status.
Keep `engine_drained` separate from
`all_claims_host_reported_quiesced`: the former is sealed engine work, the
latter is host-supplied durable evidence. The owner of each accepted claim may
record `made_record_ceremony_host_handoff` only with that claim's original
fence, owner and stable worker incarnation, plus observed time and evidence.
An exact retry returns its durable receipt; changing that id's payload
conflicts. A quiescent preflight is not liveness proof, authority to take over,
or permission to extend a lease or deadline. Resume remains a separate,
authorized lifecycle action.

When the definition itself turned out to be wrong, resuming it is the wrong
answer and cancelling loses the work. Read
`made_plan_ceremony_successor` against the published version that replaces it:
it seals nothing and reports the diff, the preflight, the evidence a successor
would start from, the disposition every outstanding claim needs and what is in
the way. Then `made_start_ceremony_successor` with a `plan_id` of your own,
one disposition per outstanding claim against the fence the plan reported, and
the steps to carry. Repeating the same request lands once; reusing the id for
different content conflicts. The old ceremony can then only be cancelled, and
carried step records say where their work actually happened.

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

### Receiving one as the working agent

Ask for your own questions with `made_pull_ceremony_agent_interventions`,
naming your execution, your process incarnation and your role. What comes
back is leased to you, not given: say what you saw with
`made_acknowledge_ceremony_agent_intervention` before you act. `received`
means you have it; `refused` and `incapable` mean you have it and will not
act, and all three are sealed in the ceremony's stream. `busy` and `timeout`
mean you do not have it, count an attempt, and return it for somebody else —
do not use them to mean "later". State `observed_at` so a retry repeats one
fact. Answer with `made_respond_to_ceremony_intervention`, naming the
`delivery_id`, `agent_execution_id` and `incarnation` you were handed, which
is what closes the route.

### Reporting whether one arrived

Read `made_get_ceremony_intervention` or `made_list_ceremony_interventions`
and report their `status_delivery` and `routes`. `delivered` means a host
holds it under a lease or an activation receipt; `acknowledged` means a named
agent said so. Never report delivery from the `intervention_delivered` or
`intervention_answered` activity labels: those are a participant's claim
about itself, and the whole point of the acknowledgement is that a claim is
not evidence. An item at `queued`, `failed` or `expired` has not reached
anybody, and saying so is the useful answer.

In hardened builds, target ids, report ids and reconsideration conditions
are unique lists with a maximum of 100. Targets may be empty; report ids and
reconsideration conditions must be nonempty. Follow the running schema.

## Run the loop as integrator

When the user asks you to drive a whole session rather than one step of it,
bind first. `made_bind_ceremony_integrator` takes a binding id you choose, the
scope (`kind: ceremony` with the session id, or `kind: system_execution` with a
run id), the seat you play, how you are reached, and your own incarnation — a
new one every time this process restarts. Keep the `binding_id` and the `fence`
from the answer; every later call presents both, and presenting a stale fence
is how a host that was replaced finds out. A live binding is refused rather
than taken over unless you pass `replace: true`.

Then loop:

1. `made_await_integrator_attention` with the scope, binding id, incarnation
   and fence. The wait is capped at 30000ms and the page at 100.
2. Read `made_get_ceremony_instance` before acting. What came with the batch
   was true when the batch was built; the item says what happened, the
   instance says what is true now.
3. `made_acknowledge_integrator_attention` with `acknowledgement: intent`,
   naming the act and the idempotency key you will run it under. This keeps
   the lease.
4. Do the work through the ordinary authorized command, under that same key.
   The loop hands out items and records what was said about them. It performs
   nothing and authorizes nothing, and being handed an item is not permission
   to do anything the seat could not already do.
5. `made_acknowledge_integrator_attention` with `acknowledgement: processed`
   and the same act, only after the effect landed. Use
   `acknowledgement: failed` with a reason when it did not.
6. Ask again.

Stop when `loop_state` is `completed`, `failed`, `blocked` or
`awaiting_human_decision`, and tell the user which. The last two are not
failures — they are the session saying a person is needed, and continuing to
poll is how a stuck session stays unreported. An empty batch with
`end_reason: wait_elapsed` means ask again; `end_reason: terminal` means stop.

Items arrive at least once: an item you already processed can be offered again
after a crash or an expired lease. Deduplicate on `delivery_id` and never
repeat an effect because the loop repeated the news.

Nobody may be able to wake you. Read `host_activation` in
`made_discover_capabilities`: with the `none` adapter the engine records the
offer and waits for you to ask, so the loop above is the only way you hear
anything. `made_list_attention_deliveries` is the paperwork when it is unclear
what was offered to whom — `delivered_to_host` there means a host was reached,
never that anybody acted.

## Inspect and report

Read session events with `made_read_ceremony_events`, passing the returned
`next_version` as the next `from_version`. Use the transcript for ordered
contributions, and journal verification for internal seals/ordering. If the
journal is not intact, report its first invalid position and do not treat
that suffix as verified evidence. Integrity does not establish the truth of
external outputs.

For a finite follow/reconnect loop, call:

```json
{"ceremony_id":"<id>","include_agent_activity":true,"after_sequence":0,"after_activity_sequence":0,"role_id":"reviewer","max_events":200,"wait_timeout_ms":1000}
```

On the next call replace the two zeroes with the two `resume_after_*` values.
Deduplicate sealed records by `event_id` and host activity by its global
`sequence`; never replay work because an observer reconnected.

For reports, select exact ceremony ids and use
`made_generate_ceremony_report`. Its `report_markdown` is the artifact and
`persisted: false` leaves saving to the host. Save only to the requested or
otherwise authorized destination and name that destination separately.
