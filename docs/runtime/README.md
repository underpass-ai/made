# Execute and resume a ceremony

A host drives the work. MADE accepts claims, validates results and records
progress. This guide describes the 0.7 contract. Fencing, durable visits and
strict list validation are absent from v0.5.0; authorization, lifecycle,
budgets, recoverable receipts and durable artifacts are part of the 0.7
boundary.
Neither a claimed step nor a successful no-op is evidence that an
agent called a tool, produced a file or changed an external system.

## Durable artifacts

Use the artifact transfer operations for outputs too large or durable to embed
in step JSON. Begin with exact digest, size, MIME type, provenance and an
idempotency key; upload independently digested chunks at the returned offset;
then commit. A retry resumes at `next_offset`. Read content in bounded chunks
and page metadata with the returned opaque cursor. Receipts link typed
`ArtifactRef` values and never storage paths or bytes.

The local adapter uses an explicit `MADE_ARTIFACT_STORE_PATH` and survives
restart on one host. A configured `MADE_POSTGRES_URL` selects the shared
transactional adapter suitable for replicas. The service leaves artifact
tools unavailable when neither is configured. Default chunks are 64 KiB,
the hard chunk limit is 1 MiB, the artifact limit is 1 GiB, and list pages
default to 50 with a maximum of 100. Tombstoning retains digest, declared
actor, authorization evidence and time for audit while denying content reads.
Actor and provenance fields are assertions, never credentials. Protected
gRPC, MCP and embedded-facade compositions authenticate the principal at their
boundary and authorize the operation against the authoritative artifact scope
before mutation; a payload cannot replace that principal or widen its grant.

## Pause, resume, cancel and deadlines

`made_pause_ceremony` stops new claims, transitions and child plans. Claims and
child plans already sealed before the pause may finish opening and adoption;
their exact fenced completions remain valid. `made_resume_ceremony` reopens
admission without shifting a deadline or lease. `made_cancel_ceremony` is
irreversible and does not implicitly cancel children or external work.

Definitions may declare ceremony, state and step timeouts. Their absolute
deadlines are sealed when the ceremony starts, a state is entered or a step is
claimed. A driver calls `made_enforce_ceremony_deadlines`; replay never reads
the wall clock. Ceremony and state expiry end the instance. Step expiry retires
that attempt and permits a retry. A result carrying the exact retired fence is
recorded once as a late observation and cannot change output, context or
terminality; a foreign fence is refused without an append.

The lifecycle phase is independent of the definition state. A ceremony can be
paused in any nonterminal state, and historical completed sessions still read
as completed even though their old snapshots contain no lifecycle field.

### Coordinated host handoff preflight

Pause controls only MADE admission. It does not stop a delegated host process,
infer that a worker is alive or lost, renew a lease, or make any already
accepted work disappear. Before an operator coordinates a resume, read
`made_inspect_ceremony_resume` and retain its bounded `next_after_claim`
cursor until it is empty. The report deliberately keeps two independent facts:
`engine_drained` is derived from the sealed engine state, while
`all_claims_host_reported_quiesced` is the latest durable declaration from
each exact host claim that is still in flight. Completed, failed and replaced
historical claims remain visible in the cursor but do not require a new host
acknowledgement. Neither fact substitutes for the other.

The host that owns an accepted claim may journal
`made_record_ceremony_host_handoff` with its original `claim_fence`, owner,
stable worker `incarnation`, observed time and evidence reference. A
`quiesced` declaration for every in-flight claim makes
`coordinated_resume_ready` true only while admission is paused and no absolute
deadline is overdue. Separately, a sealed `engine_drained` fact satisfies this
coordination condition without requiring a host declaration. An expired
in-progress claim is not ready merely because it has a host declaration: its
explicit recovery disposition remains necessary. This is a coordination
signal, not an authorization, liveness verdict or takeover permit; resume
remains a separate lifecycle action.

Handoff identities are durable receipts: an identical retry returns the
original record without appending or changing clocks. A reused identity with a
different payload conflicts. A new declaration carrying a replaced owner,
fence or worker incarnation is refused without mutation. The preflight only
reports absolute deadlines and receipt/reconciliation evidence; it never
extends a deadline or lease.

### Revise the definition and hand off to a successor

A paused ceremony can be resumed under the definition it started with, or
cancelled. Neither answers the case where the definition turned out to be
wrong and work has already been done under it. Resuming replays a definition
nobody believes in; cancelling and starting again strands sealed evidence and
the external effects behind it, and leaves no record of why a second ceremony
exists. Succession is the third answer.

| | resume the same definition | open a successor |
|---|---|---|
| definition | unchanged; the instance stays bound to its pin | a different published version, pinned in both ceremonies |
| completed work | stays where it is | carried by reference onto the successor's steps, never copied |
| outstanding claims | keep their fences and continue | each needs an explicit disposition against the fence the plan saw |
| deadlines | absolute and unchanged | sealed afresh from the successor's own definition |
| budget | the same account | fresh; `transfer_remaining` is refused in this release |
| the old ceremony | goes on | can only be cancelled; resume is refused with `superseded_by_successor` |
| audit | one stream | two streams that name each other |

Read `made_plan_ceremony_successor` first. It seals nothing and answers with
the definition diff, the whole resume preflight, the evidence a successor
could start from, the disposition every outstanding claim would require, the
changes that would strand work already completed here, and the blockers in the
way. `ready` is true only when the ceremony is paused, has not already sealed
a handoff, and nothing the successor strands has been completed here.

`made_start_ceremony_successor` then seals the handoff in the paused ceremony
and opens the successor it names, in that order. The `plan_id` is the caller's
and is what makes the whole operation retryable: the successor's id derives
from it, so a repeat of the same request seals nothing new and opens nothing
twice, while the same id asking for different content conflicts. A crash
between the two appends is resumable — the retry finds the seal and either
makes the opening or verifies the one already there. A stream holding the
successor's id with different content is a foreign ceremony and is refused.

Evidence is referenced, not copied. A carried step record keeps
`carried_from`, naming the predecessor, the step, the sealed event, its record
hash, the state visit and the attempt, so a step the successor never ran is
never presented as if it had been. A step the diff strands cannot carry, and a
step the successor does not declare fails the plan by name.

Authorization is checked twice and separately: on the ceremony handing off,
and on the definition the successor would run. Planning a successor does not
grant the right to start one.

## Claim → work → complete

1. Inspect the instance and choose a claimable step.
2. Call `made_claim_ceremony_step` with that step and the caller identity.
3. Retain the accepted claim response and its opaque `claim_fence` before
   starting external work. Completion requires this exact value.
4. Perform the step with the host's authorized worker and tools.
5. Call `made_complete_ceremony_step` with observed status and output,
   evidence references and that claim's unchanged `claim_fence`.
6. Refresh the instance. Apply only an enabled transition; obtain an explicit
   human decision when a human guard blocks it.

Use the exact schema from `tools/list`. The fence requirement is a deliberate
protocol change in 0.6.0; see [migrations](../migrations/README.md).
Never fetch a replacement worker's current fence to attach an old result.

### Renew long delegated work

Source builds containing #202 expose `made_renew_ceremony_step_lease`; the
released 0.7.8 catalog does not. Check the active `tools/list` and discovery
version rather than assuming that a checkout upgrades the installed binary.
The Rust facade uses `renew_step_lease`, the client uses the same method, and
the RPC is `RenewCeremonyStepLease`.

Before effective expiry, the original authenticated host sends:

```json
{"ceremony_id":"session-17","step_id":"work","claim_fence":"<accepted claim fence>","lease_owner_id":"host-1","renewal_id":"heartbeat-1","lease_ttl_ms":60000}
```

Use a new renewal id for each heartbeat. After a lost response, retry exactly
the same id and payload: MADE returns the original receipt and does not extend
again. Reusing the id with different payload is a conflict. The receipt's
`effective_lease_expires_at` is the expiry accepted by that request; inspect
the step's current field for later renewals. A replay after completion is
historical evidence, never permission to resume work. The sealed
`step_lease_renewed` event records the request, original owner/fence and time.

Both the renewal action and original claim authority must remain authorized.
The logical lease owner cannot substitute for authenticated identity. A new
renewal rejects expired or replaced claims, cancellation, terminal state and
overdue absolute deadlines. Pause closes admission but permits accepted work
to renew and finish; it does not shift deadlines. Renewal is capped by the
earliest step/state/ceremony deadline and retains the original lease, attempt,
operation and budget reservation. It does not create a worker or prove that
an external agent is alive. On refusal, stop further external work and use the
explicit recovery path; never borrow a successor's fence.

## Concurrent states and host fan-out

For a one-shot `made_run_ceremony`, the application driver automatically runs
siblings in a concurrent state as a bounded batch. It first seals every claim
in the batch, then invokes their handlers concurrently, records every accepted
result even when a sibling fails, and reloads the fold before it chooses an
early join, another batch or a transition. The effective width is the lower of
the definition's `max_parallel` (default 3) and the runtime ceiling
`MADE_MAX_PARALLEL` (default 8). Sequential states retain declaration-order
execution. Deliberation proposal seeding remains sequential.

For delegated execution, inspect all `claimable_step_ids` and let the host fan
them out to its own workers or subagents. Each worker must retain its own claim
receipt and fence; complete accepted claims in whatever order the work really
finishes, then refresh the instance before applying a join transition. Do not
cancel an accepted sibling merely because an `any` or counted join became true:
a live lease blocks the transition until that work completes or expires.

Separate hosts may share the durable SQLite store. Optimistic append and the
state's effective capacity decide which distinct claims land. The MCP protocol
exposes claim, renewal and completion operations; an MCP call does not create workers,
processes or subagents. The service binary can separately install the explicit,
opt-in [ceremony worker daemon](worker-daemon.md). When that daemon is disabled,
the calling host owns worker lifecycle, authorization, tool access and external
idempotency.

With a verified server-owned `CeremonyStepHandlerPort`, use
`made_run_ceremony_step`; the application retains the accepted fence through
the handler and completion retries. Use one-shot `made_run_ceremony` only
when all required handlers are real and no later human decision is needed.
The bundled default handler may be a no-op.

## Child execution and recovery

A claimed spawn step is application-owned work. Both automatic drivers call
the child orchestrator instead of the configured step handler. The
orchestrator seals one plan, opens or verifies every deterministic child
stream, then completes the parent step with the child group and ids. A crash
can leave a planned or adopted group with only some children open; retry and
recovery continue the same plan and reject a stream whose opening differs.

Child completion is a separate fact. The accept operation names the child's
exact `CeremonyCompleted` event. MADE verifies the complete audit chain, the
event hash, the sealed publication and lineage before appending the parent
acceptance. `all`, `any` and quorum child guards count these accepted facts.
Completion is irreversible: a child that later accepts one of its own late
siblings remains terminal, and callers continue to use the original terminal
event id.

The service listens for NATS ceremony notifications to reduce recovery
latency, but notification payloads and delivery are not authoritative. A
named durable cursor scans the global event feed on startup, on broker wake
and periodically. It acknowledges a position only after its child effect has
landed; another process may hold the cursor lease temporarily. Embedded
composition drains the same recovery use case without opening a broker
connection.

## Leases, retries and fences

A claim has an exact execution identity and lease. If another worker takes
over expired work, the old worker must not append its result to that new
claim. The opaque SHA-256 fence binds the accepted claim's execution
coordinates, including `state_visit`. The response's instance and audit identity
describe that claim at its accepted stream version, even if another worker
replaces it before the response arrives. Rust also exposes that version through
`StartCeremonyStepOutput::version()`; RPC and MCP do not return it as a separate
field. Missing or malformed fences are invalid requests; a valid but
wrong or superseded fence is refused. Refusal appends no event and changes
neither output/context nor the replacement lease.

Expiry alone does not reject the original completion until another claim
replaces it. Fencing prevents stale result acceptance; it cannot cancel an
external effect that already happened, and it is not exactly-once execution.
Use host-side idempotency for external writes. A completion retry must retain
the original claim identity, not rebind itself to the latest snapshot.

## Iteration and visit coordinates

These are distinct dimensions of execution:

- `attempt` is a technical attempt within a semantic step execution.
- `iteration` identifies bounded successful repetition of a step.
- `state_iteration` identifies repetition of all work within the current state.
- `state_visit` identifies entry into a state, including later cycle entries.

The integrated state-visit contract gives re-entered work
a fresh execution coordinate and records the exact reset set in the sealed
transition event. Older event payloads keep their historical meaning; new
folding must not invent reset instructions for them. Event identifiers must
remain distinct when a step name is reused in another visit.

Do not confuse a repeated `next_step_id` with duplicated work: examine its
visit, iteration, attempt and claim. A bounded repeat can expose the same step
again after successful output; a cycle can enter its state again. Reopen
must reconstruct these coordinates from the event stream and snapshot tail.

## Humans and participant interventions

Approve a human guard only for a person's actual decision. The actor/role
kind records a declaration; it does not authenticate a human. Deferral keeps
the session paused with concrete `reconsider_when` conditions.

An intervention records a participant's question, investigation or action
request. Empty or omitted `target_role_ids` addresses the whole table;
explicit targets address those roles. Naming
`target_agent_execution_id`, `target_incarnation` and `target_role_id`
together addresses one live agent instead: the incarnation is required
because a replacement process reusing an execution id is a different agent,
and a question addressed to a process dies with it unless the delivery terms
say to follow. A configured evidence source may
collect a read-only response. Otherwise the host obtains the real response
and records it. An action intervention does not bypass a guard or authorize
an external mutation.

### Getting it to the agent, and knowing it arrived

`made_get_ceremony_intervention` and `made_list_ceremony_interventions`
report one projected status per item, computed from the sealed stream and
the delivery ledger together:

| status | what is behind it |
|---|---|
| `recorded` | sealed in the stream; no route was ever opened |
| `queued` | waiting in the ledger for a host to take it |
| `delivered` | a host holds it: a lease it took, or an activation receipt |
| `acknowledged` | a named agent said it saw it, sealed in the stream |
| `responded` | it was answered |
| `closed` | the requester closed it |
| `failed` | attempts ran out, or a host took it and declined |
| `expired` | it stopped being worth making; the cause says why |
| `unsupported` | every destination it was addressed to is gone |

A working agent asks for its own questions with
`made_pull_ceremony_agent_interventions`, which verifies its claim against
the journal and hands out an expiring lease. A lease is an offer, not a
receipt: it says the engine handed the item over and no more. The agent then
says what it saw with `made_acknowledge_ceremony_agent_intervention`.
`received`, `refused` and `incapable` are statements about the item and are
sealed in the ceremony's stream as `InterventionDeliveryAcknowledged`;
`busy` and `timeout` are statements about the host, count an attempt and put
the offer back for whoever can take it. State `observed_at` if you might
retry, so a repeat is the same fact rather than one that differs by a clock
reading. An answer that names its `delivery_id`, `agent_execution_id` and
`incarnation` is checked against the ledger before it is sealed, and closes
the route afterwards.

**What a delivery does not prove.** `made_report_ceremony_agent_status`
still accepts the activity labels `intervention_delivered` and
`intervention_answered`. They remain what they always were: a claim by the
reporter about itself. They are not an input to the status table above, and
nothing derives delivery from them. The evidence that an intervention
reached somebody is the sealed acknowledgement, which names who saw it and
when; the evidence that it did not is a route sitting visibly at `queued`,
`failed` or `expired`.

Everything else about the journey — queueing, leasing, attempts, expiry —
lives in the delivery ledger and never in the sealed stream. A transport
retry is bookkeeping, not something the ceremony decided.

**A supervisor may ask without holding a seat.** Passing `supervisor`
records the item as asked by a role derived from that principal, which no
definition can declare, and requires ambient authorization naming the same
principal. Authority to ask is not authority to mutate: the supervisor gains
no other action, and closing the item remains the requester's.

New report ceremony ids, reconsideration conditions and intervention target
ids are unique bounded lists of at most 100 items. Report ids and reconsideration conditions must be nonempty;
intervention targets may be empty for the whole table. Legacy deserialization
remains compatible; new commands validate strictly. Uniqueness follows
whitespace normalization. Report ids and conditions preserve caller order;
scoped recipients use an ordered set. Invalid input rejects the entire command
without truncation or silent deduplication.

## Integrator loop: attention events and host activation

An integrator is a host bound to one ceremony or to one run of a composed
system, driving it from outside. What it is owed is derived from the journal,
never from anything a writer remembered to enqueue: the engine projects
attention for every live binding a ceremony concerns after each append, and
each binding walks the global feed through a durable cursor of its own
(`attention:{binding_id}`).

Being told after an append is a wake-up, and a wake-up can be missed — the
process was down when the append landed, or it died between the two. The
journal and the cursor are the authority, so the projection also runs on the
read path: `await_integrator_attention` projects for the binding it has just
fenced, and `list_attention_deliveries` projects for the binding asked about,
or for every live one when none is named. A restart therefore recovers by
being asked an ordinary question, with no background sweeper deployed.

A projection that cannot run never fails the thing it ran for. After an append
the failure is logged and the ceremony stands; before a read the ledger answers
as it stands, because what earlier rounds offered is still there. The cursor is
what makes both safe: the position was not acknowledged, so the work is owed
again.

Waking a host is separate from offering it work. `HostActivationPort` has a
`none` adapter, which is the default: the offer sits in the ledger until the
host pulls. A delivery that never moved reads differently depending on which
silence it is, and the ledger records which.

The one alternative is `command`: the operator names a single command in
`MADE_HOST_ACTIVATION_COMMAND` and the engine runs it with the envelope as JSON
on its standard input and a cleared environment plus the destination, the host
kind and the delivery id. Nothing arriving in the envelope picks what runs.
Exit zero is a transport receipt; any other exit, and a command that does not
answer in time, fails the delivery, which is then offered again. See
[the operator note](../operations/host-activation.md).

### What a host is told, and what it says back

Nine kinds of attention are derived: `result_available`, `review_rejected`,
`step_failed`, `blocked`, `human_decision_requested`, `deadline_exceeded`,
`inactivity_detected`, `intervention_requested` and `ceremony_ended`. Each one
names something that happened in the journal; none of them says what to do
about it. An item carries a thin context — the ceremony's phase, its current
state, the steps that can be claimed and the guards waiting on a person — and
the documented sequence has the integrator read the instance again with
`made_get_ceremony_instance` before acting, because what travelled with the
batch was true when the batch was built.

A host answers in two calls, in that order. `made_acknowledge_integrator_attention`
with `acknowledgement: intent` records what the host is about to do and the
idempotency key it will do it under, and keeps the lease; the host then runs the
ordinary authorized command; only afterwards does `acknowledgement: processed`
close the item. `acknowledgement: failed` counts an attempt and may offer the
item again. A single call after the fact could not tell a crash mid-effect from
an effect that never started, which is the difference between resuming and doing
the work twice.

Acknowledging transport is not acknowledging processing. An activation receipt,
and the `delivered_to_host` state it produces, say only that a host was reached.
What says anybody acted is an acknowledgement naming an act.

### At least once, and stopping

Delivery is at least once. An item that was leased and not answered comes back
when the lease expires, and an item whose processing crashed after the effect
landed is offered again. Hosts deduplicate on `delivery_id`, which is derived
from the item and the target rather than minted per offer, and carry their own
idempotency key into the command they run.

Both ends are bounded. `wait_timeout_ms` is capped at 30000 and defaults to
1000; `limit` is capped at 100. Every batch carries `loop_state`, the empty ones
included, because "nothing yet" and "nothing ever" are otherwise identical from
outside: ask again while `end_reason` is `wait_elapsed`, and stop when
`loop_state` is `completed`, `failed`, `blocked` or `awaiting_human_decision`.
The last two are not failures — they are the loop saying a person is needed.

### A host that cannot be woken

`made_discover_capabilities` reports `host_activation`, with the adapter this
deployment composed and the tool a host follows its scope with instead. Every
composition in this build installs the `none` adapter, so the honest answer for
a host asking whether it can wait to be knocked on is no: bind, and ask. The
ledger still records every offer, which is what lets an operator read
`made_list_attention_deliveries` and see work that was derived and never handed
over.

## Resume without replaying side effects

List instances before creating a replacement after context loss. Read the
matching instance and its stream. Published definitions allow rehydration;
`rehydratable: false` identifies a snapshot whose definition is unavailable.
Do not report a reconstructed successor as the original session.

If a host loses its claim receipt, establish which claim performed the work.
Do not borrow another claim's identity. When recovery cannot establish that
identity, wait for lease expiry and claim new work with a new key, applying
external idempotency where necessary.

## Read the record

`made_read_ceremony_events` pages a session's sealed stream; `from_version`
is the position already read and `next_version` is the next cursor to send.
`made_stream_ceremony` replays after `after_sequence` and then follows the
same sealed store for a bounded wait. It returns the last sequence actually
delivered as `resume_after_sequence`; send that value unchanged on the next
call. `max_events` defaults to 200 and is limited to 1000.
`wait_timeout_ms` defaults to 1000, accepts 0 for replay only and is limited to
30000. Completion says `terminal`, `event_limit` or `wait_elapsed` explicitly.

Set `include_agent_activity` to receive a current filtered agent snapshot plus
typed host activity. `role_id`, `step_id` and `agent_execution_id` filter what
is returned but do not create private cursor domains: the global
`resume_after_activity_sequence` advances across filtered records. Reconnect
with both resume cursors unchanged. A cursor older than the retained activity
window is rejected explicitly. The feed labels host assertions separately from
sealed engine transitions and accepted results; summaries and evidence links
are bounded, while raw logs, token deltas, credentials and private reasoning
are outside this contract. Idle identical heartbeats may be coalesced, but
blockers and requested input are retained. The in-process activity adapter
retains at most 1000 entries per ceremony and stream channels apply bounded
backpressure; dropping an observer only aborts its producer, not ceremony work.
On a fresh activity cursor, the snapshot is captured at the returned activity
head and only later updates follow, so folded state is not replayed twice. Its
`complete` flag is false when the requested bound could not contain the whole
filtered roster.

Status reporters use the public labels `checkpoint_available`,
`intervention_delivered`, `intervention_answered`, `input_requested`, `failed`
and `heartbeat` to select those dedicated activity kinds. Blocker-field changes,
host incarnation replacement, stale liveness and finished execution are typed
from their structured fields rather than inferred from prose.

The gRPC RPC is a real server stream. MCP collects that bounded stream into one
finite tool response because stdio has no reliable live-progress channel. A
250 ms poll discovers writes made through another process; this is a polling
interval, not a maximum delivery-latency promise. Store work, scheduler load
and a slow consumer can add delay. The wait timeout bounds how long the
producer waits for more events after replay. Frames already read are delivered
with backpressure, so a slow client can make the call last beyond the timeout
rather than losing them.

`CeremonyCompleted` ends the current follow request as soon as that record is
delivered. A later request from its cursor can still read facts accepted after
completion, such as a late child completion. The journal remains terminal even
when its latest record has another type; terminality is derived from sealed
history, not from the head record alone.

`made_pull_ceremony_events` reads the global feed, and consumer acknowledgement
tracks delivery progress. Those are different cursor domains.

`made_get_ceremony_transcript` projects completed contributions from the
stream. `made_verify_ceremony_journal` checks seals and ordering, not the
truth of external claims. `made_generate_ceremony_report` projects selected
persisted sessions into Markdown. Its `persisted: false` means the host must
save the returned artifact when asked to do so.
