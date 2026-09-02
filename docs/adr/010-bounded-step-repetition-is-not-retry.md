# ADR-010: Bounded step repetition is not retry

Status: Accepted

Implementation status last verified: 2026-09-02

## Context

Some working sessions cannot decide their full number of steps in advance. A
coordination step may refresh evidence successfully, yet report that its stop
criterion is still false and need another pass. The blueprint catalog already
described this as “repeat until”, but the executable ceremony model stored one
record per step and treated `Completed` as final. Cyclic state transitions did
not solve it: a completed step could not execute again, and the ceremony-level
transition cap only stopped the resulting dead end.

Retry is the wrong abstraction. A retry means that work failed, expired, or
must be re-driven without claiming a new semantic result. A successful
evidence refresh followed by another refresh is new meeting history. Giving
both the same coordinate would overwrite the output that explains why the
next pass happened and make audit events ambiguous.

Any data-dependent loop also needs a bound. A condition controlled by an
agent or external observation cannot be assumed to become true.

## Decision

A ceremony step may declare one optional repeat policy:

```yaml
repeat:
  max_iterations: 4
  until:
    output_field: ready
    equals: true
```

The condition compares one top-level structured output field with an exact
JSON value. A missing field is false. `max_iterations` is mandatory, positive,
and capped at 1000.

Every step execution has two independent coordinates:

- `iteration` starts at 1 and advances only after a successful output fails
  the repeat condition;
- `attempt` belongs to retry and restarts at 1 for every semantic iteration.

The aggregate keeps preceding successful iteration records in durable history
and exposes the current record separately. Audit fact identities include both
coordinates. Transcripts include every successful iteration so the next pass
can reason from the result that caused it.

A transition out of the state is disabled while any repeated step has not met
its condition. Once the condition is true, existing guards apply normally. If
the final permitted iteration remains false, the step exposes
`repeat_limit_reached`; one-shot execution records the stable `repeat_limit`
outcome and returns an error rather than spinning or transitioning.

Definitions without `repeat` retain their serialized shape. Instances written
before this decision deserialize missing iteration coordinates as iteration 1
and missing histories as empty.

## Consequences

- Authors can express data-dependent working-session loops without expanding a
  fixed number of duplicate states and steps.
- One-shot, server-owned incremental, and delegated-host execution use the
  same aggregate rule and expose the same next-step behavior.
- Consumers must distinguish `iteration` from `attempt`; treating both as
  retries produces false reliability signals.
- The first condition vocabulary is intentionally small. Nested field paths,
  compound predicates, and cross-step expressions require an explicit future
  contract rather than ad-hoc strings.
- Each semantic iteration retains a record and transcript contribution, so a
  high bound has storage and context cost. The hard maximum limits that cost
  but does not replace sensible authoring.
