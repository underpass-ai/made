# ADR-015: Concurrent states and join guards

Status: Accepted (2026-09-17)

Implementation: B1 and B2 were implemented by P1 in #122
(`e0dc475b014aab6c629ded2756fbbc4d94e251f8`). Concurrent drivers,
aggregation and fan-out observability remain outside this decision's
implementation slice as B3–B6.

## Context

Before this change, a state exposed its next step in declaration order and
independent roles could not claim different steps at the same time. Making a driver launch
more tasks would bypass that aggregate rule and would give the editions
different execution semantics.

## Decision

A state may declare `execution: concurrent`; omission means sequential and
preserves existing definitions. Concurrency is between steps of one state.
Each step retains its own lease, attempt, semantic iteration and sealed events.
Static steps in a concurrent state must have distinct role owners. Dynamic
steps may have overlapping allow-lists; claims enforce distinct actual sealed
roles within the active state iteration. Analysis rejects duplicate static
owners and concurrent states without an outgoing join guard. It warns when
a concurrent state can use more than three roles.

The guards `any_step_completed` and `steps_completed: n` count successful
steps in the current state iteration. The latter requires a positive `n` no
larger than the state's step count. `all_steps_completed` remains valid. The
aggregate evaluates joins; a driver cannot declare a join complete itself.
Existing human guards and bounded step repetition still apply. An unmet step
repeat prevents leaving the state, except for the explicit exhausted-repeat
route defined in ADR-016. A transition cannot abandon a live lease; callers
must complete it or let it expire before taking an early join.

`claimable_step_ids` is an additive view on all four surfaces, in definition
order. `next_step_id` remains the first claimable step for existing clients.
In sequential states only the next step is claimable. In concurrent states
several distinct steps may hold live leases, bounded by `max_parallel`.
Optimistic stream revision checks decide competing claims; a conflict reloads
and re-evaluates eligibility rather than bypassing the aggregate.

`max_parallel` defaults to **3 per definition**. Its validated range is 1–8.
The server's `MADE_MAX_PARALLEL` ceiling defaults to **8**, accepts 1–8 and
bounds the effective concurrency by the lesser of the definition and server
limits. An invalid configured ceiling is a configuration error. Embedded
execution applies the definition limit; a Rust host may impose a lower limit.
This bounds simultaneous leases and later driver scheduling, not the number
of roles an author can declare.

Every field and guard lands in proto, MCP over gRPC, embedded MCP and the
Rust facade in the same PR, including design input, YAML, views and examples.
Domain identifiers, limits and join counts are validated values. Serializers
and wire DTOs stay at boundaries. Missing execution fields in earlier stored
state retain sequential semantics.

## Verification

#122 proves two distinct claims and completions in either order, same-step
lease exclusion, capacity exhaustion and expiry, optimistic conflict/retry,
all/any/count joins, sequential compatibility and event replay. Its real store
race admits only the capacity-bounded winners. The parity session drives the
same claimable ids, claims, completions and readiness through both MCP
backends, while direct proto and Rust-facade tests cover the remaining two
surfaces. Definition analysis and YAML/design round trips cover the declared
refusals, warnings, defaults and limits.

## Consequences

The aggregate supports concurrent hosts and multiple live step leases before
an automated concurrent driver ships. The one-shot driver remains sequential.
B3–B6 (drivers, aggregation and fan-out metrics), C1 and the complete D1–D5
pattern scenarios remain work for corte 4. This ADR does not claim those paths
run concurrently end to end.
