# ADR-016: Bounded definition primitives

Status: Accepted (2026-09-17)

Implementation: bounded state repetition is implemented by P2 (#128); output and
exhausted-repeat guards are implemented by P3 (#116). Role binding and context
writes are implemented by P5 (#130); transition budgets remain in the separate
P4 slice.
Extends ADR-010; its step repeat contract remains unchanged except for an
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

### Transition budgets and cycles

Definitions may declare positive `max_transitions` and `max_bounces` limits.
`max_transitions` caps the total transitions of one instance; `max_bounces`
caps repetitions of a particular declared edge (source, trigger, target).
Counters derive from transition events and survive reopening. A command that
would exceed a cap is refused before append. Analysis rejects a cyclic graph
without a declared cap and warns when a cyclic graph contains no human state.
A bounded state repeat has its own iteration budget and does not consume a
transition merely by starting the next state iteration.

### Fragments and scope

Reusable YAML fragments live under `api/examples/ceremonies/fragments/` and
are embedded from `made-app` with `include_str!`. The impact planner must route
every such external source to the Rust jobs that compile it. P6 introduces
`pattern:` in design input, catalog discovery and `roundtable_fixed_order`,
a fixed sequential example that uses no new engine primitive. Encoding still
belongs to an adapter; the embedded fragment is definition input data.

## Verification and consequences

P2–P6 each include the four surfaces, design schemas, YAML and documentation.
Tests must cover malformed input, missing output, refusal without appended
events, bounds, replay/reopen and both MCP editions. P5 updates
`EVERY_EVENT_TYPE`, pinned event contracts, shared renderers, the parity session
and the report golden for `ContextWritten`.

These primitives and concurrent states complete phase 3a. Drivers, aggregation,
`broadcast_collect`, full D1–D5 patterns and composition remain for corte 4;
accepting this ADR is not evidence that those patterns run end to end.
