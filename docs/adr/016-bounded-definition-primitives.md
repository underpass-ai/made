# ADR-016: Bounded definition primitives

Status: Accepted (2026-09-17)

Implementation: bounded state repetition was implemented by P2 (#128); output
and exhausted-repeat guards by P3 (#116); transition budgets and cycle
analysis by P4 (#131,
`8785ac5edfb5a8e574430a9d47714d2115ef6d71`); dynamic role binding and atomic
context writes by P5 (#130); fragment infrastructure and the fixed sequential
`roundtable_fixed_order` precursor by P6 (#114). Together with P1 (#122), these
deliver all seven generic definition primitives accepted for Phase 3a.
Extends ADR-010; its step-repeat contract remains unchanged except for an
explicitly guarded exhaustion exit.

## Context

Patterns need to repeat a group of steps, inspect another step's output,
route to an allowed role and carry declared results into context. These are
shared definition rules, not pattern-specific branches in the core. Cycles
need a durable bound before any such pattern can execute safely.

## Decision

### State repetition

A state's optional `repeat` creates a **state iteration coordinate**, starting
at 1, and reruns all its steps under that coordinate. It is not a self-transition.
The policy has a positive `max_iterations` no greater than 1000 and an `until`
condition naming `step`, `output_field` and the exact JSON `equals` value.
The named step belongs to that state. For example:

```yaml
repeat:
  max_iterations: 4
  until:
    step: check
    output_field: approved
    equals: true
```

The aggregate evaluates the condition after the state's work completes. False
starts the next state iteration if budget remains; true allows ordinary exit
guards. Missing output is false. Exhaustion produces a stable bounded outcome,
never an implicit restart. No live lease crosses a state-iteration boundary.
Step repeat and retry remain separate coordinates: step iteration and attempt
restart within each state iteration. Successful prior work remains in history
and the transcript. Events and views carry the state coordinate additively;
old records with no coordinate mean iteration 1.

P2 implements this boundary as one sealed `StateIterationStarted` event in the
same append as the completion that closes the prior iteration. Replay,
snapshot-tail loading, SQLite reopen, history, transcripts and all four public
surfaces preserve the coordinate. Presence-backed event payloads keep pre-P2
event hashes and schema-version-1 records byte-for-byte compatible.

### Output and exhaustion guards

`output_field:<step>:<field>=<json>` compares a declared step's latest successful
output in the active state iteration with an exact JSON value. The field is
a top-level key; missing output or field is false. JSON types are significant.
Unknown step ids, malformed expressions and invalid JSON fail analysis or
construction. There is no expression evaluator or nested-path language.

`step_repeat_exhausted:<step>` is true when that step completed its final
permitted iteration with its stop condition still false. That explicit route
may leave the state despite this step's exhausted repeat; it does not waive
other unmet repeats, required completion or join guards (including those that
block live leases), human approval or open interventions.
Ordinary transitions retain ADR-010's refusal on unmet repetition. Each new
guard variant owns a source file and validated value, rather than expanding
an untyped expression switch.

P3 implements both typed guards on the active record set. Its exhaustion
waiver is scoped to the named step and exact transition; it never waives a
different repeat, the repeated-state all-work barrier, a live lease, human
approval or an open intervention.

### Role binding and context writes

A step may declare `role_from: context.<key>` and an explicit non-empty
`allowed_roles` list. Every allowed role exists in the definition, with no
duplicates. Claim resolves the context's string role id against this list.
Missing values, non-string values and roles outside the list refuse the claim
before an event is appended. The claimed role is sealed so completion and
replay do not re-resolve it against later context. Static role binding remains
the default. Dynamic role binding must preserve concurrent role separation.

`context_writes` maps destination context keys to top-level step output keys.
Completion validates all declared source fields and seals `ContextWritten`
**in the same append as `StepCompleted`**. A missing source refuses the entire
completion, with no partial writes or completion event. Apply/fold is the only
path that changes aggregate context. Retried commands cannot duplicate the
write; concurrent completions use the existing revision check and retry.
The event has the same journal, renderer, verification, report and parity
coverage as every other ceremony event. Definitions omitting these fields
preserve their existing shape and behavior.

P5 (#130) implements these fields across YAML, direct gRPC, MCP over gRPC,
embedded MCP and the Rust facade. Claim seals the resolved role before work
starts. A successful completion validates the whole patch first, then seals
`StepCompleted -> ContextWritten -> [StateIterationStarted]` in one optimistic
append; replay is the only writer of aggregate context.

### Transition budgets and cycles

Definitions may declare positive `max_transitions` and `max_bounces` limits.
`max_transitions` caps the total transitions of one instance; `max_bounces`
caps applications of a particular declared edge (source, trigger, target),
including the first application.
Counters derive from transition events and survive reopening. A command that
would exceed a cap is refused before append. Analysis rejects a cyclic graph
without a declared cap and warns when a cyclic graph contains no human state.
A bounded state repeat has its own iteration budget and does not consume a
transition merely by starting the next state iteration.

For cycle analysis, a component is human-controlled when any transition out of
one of its states, including an edge that exits the component, requires a
`HumanApproval` guard. Either positive cap is sufficient to admit a cycle.
Changing a definition to add or lower a cap may strand a running instance;
removing or raising one does not.

Returning to a state currently preserves that state's existing step records;
it does not create a new durable state visit or rerun completed steps. Cyclic
definitions that require fresh work on every visit therefore await the durable
visit/reset contract tracked in [#129](https://github.com/underpass-ai/made/issues/129).

P4 (#131) implements both typed caps from sealed transition history and
rejects an unbounded strongly connected component. Refusal occurs before an
append; replay, snapshots, imported history and process restart derive the
same counts. A state-iteration boundary consumes no transition budget. The
caps do not define state re-entry semantics: durable state visits remain
tracked in #129 before cyclic D3/D4 patterns can be claimed.

### Fragments and scope

Reusable YAML fragments live under `api/examples/ceremonies/fragments/` and
are embedded from `made-app` with `include_str!`. The impact planner must route
every such external source to the Rust jobs that compile it. P6 (#114)
implements the fragment catalog, checked-in source routing, typed `pattern:`
input and `roundtable_fixed_order`. That preset is D1 v0: fixed, sequential
speaking turns over existing primitives. It is not full D1, which still
requires manager-selected speakers, dynamic role binding, state repeat and its
complete E2E. Encoding remains an adapter concern; the embedded fragment is
definition input data.

## Verification and consequences

P2–P6 cover their fields on proto, MCP over gRPC, embedded MCP and the Rust
facade, with design schemas and YAML where the capability is authored. Their
focused code evidence covers malformed input, missing output, refusal without
append, bounds, optimistic races, replay/reopen, legacy digests and sealed
hashes, and both MCP editions. The exact composed candidate, repository gates
and operator evidence are recorded by the Phase 3a closure PR #130.

Together with ADR-015, these changes complete Phase 3a's generic primitive
foundation and claimable concurrency as of 2026-09-18. They do not complete
Phase 3 as a whole. Automated concurrent drivers (B3–B6), aggregation,
in-ceremony `broadcast_collect` (C1), complete D1–D5 pattern fragments/E2Es,
local-edition councils (F5) and composition remain deferred to corte 4. #127
still tracks completion fencing, and #129 must define durable state visits
before cyclic D3/D4 patterns rely on re-entry.
