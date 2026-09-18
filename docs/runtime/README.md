# Execute and resume a ceremony

A host drives the work. MADE accepts claims, validates results and records
progress. This guide describes the 0.6.0 contract; fencing,
durable visits and strict list validation are absent from v0.5.0.
Neither a claimed step nor a successful no-op is evidence that an
agent called a tool, produced a file or changed an external system.

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
exposes claim and completion operations; it does not create workers, processes
or subagents. The host owns their lifecycle, authorization, tool access and
external idempotency.

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
explicit targets address those roles. A configured evidence source may
collect a read-only response. Otherwise the host obtains the real response
and records it. An action intervention does not bypass a guard or authorize
an external mutation.

New report ceremony ids, reconsideration conditions and intervention target
ids are unique bounded lists of at most 100 items. Report ids and reconsideration conditions must be nonempty;
intervention targets may be empty for the whole table. Legacy deserialization
remains compatible; new commands validate strictly. Uniqueness follows
whitespace normalization. Report ids and conditions preserve caller order;
scoped recipients use an ordered set. Invalid input rejects the entire command
without truncation or silent deduplication.

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
