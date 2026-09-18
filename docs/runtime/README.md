# Execute and resume a ceremony

A host drives the work. MADE accepts claims, validates results and records
progress. This guide describes the 0.6.0 contract; fencing,
durable visits and strict list validation are absent from v0.5.0.
Neither a claimed step nor a successful no-op is evidence that an
agent called a tool, produced a file or changed an external system.

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

With a verified server-owned `CeremonyStepHandlerPort`, use
`made_run_ceremony_step`; the application retains the accepted fence through
the handler and completion retries. Use one-shot `made_run_ceremony` only
when all required handlers are real and no later human decision is needed.
The bundled default handler may be a no-op.

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
`made_pull_ceremony_events` reads the global feed, and consumer acknowledgement
tracks delivery progress. Those are different cursor domains.

`made_get_ceremony_transcript` projects completed contributions from the
stream. `made_verify_ceremony_journal` checks seals and ordering, not the
truth of external claims. `made_generate_ceremony_report` projects selected
persisted sessions into Markdown. Its `persisted: false` means the host must
save the returned artifact when asked to do so.
