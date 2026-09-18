# Author a ceremony

A ceremony definition is a versioned state machine with roles, steps,
transitions and guards. A session is one execution of that definition with
its own context and event stream. Define the decision or artifact first,
then the work that must produce it.

## A complete definition

Save this as [two-checks.yaml](examples/two-checks.yaml). Two independent
inspections become claimable in `CHECKING`; the join counts completed work
in that source state. `max_parallel` permits both claims, but the host still
schedules and performs them. The handler names identify host integrations,
not bundled workers.

```yaml
version: "1.0"
name: two_checks
description: Two independent inspections before a review can close
inputs:
  required: [change_summary]
states:
  - id: CHECKING
    initial: true
    execution: concurrent
  - id: CLOSED
    terminal: true
transitions:
  - from: CHECKING
    to: CLOSED
    trigger: finish
    guards: [both_done]
steps:
  - id: inspect_api
    state: CHECKING
    handler: api_review
  - id: inspect_storage
    state: CHECKING
    handler: storage_review
guards:
  both_done:
    type: automated
    check: "steps_completed:2"
roles:
  - id: API_REVIEWER
    allowed_actions: [inspect_api, finish]
  - id: DATA_REVIEWER
    allowed_actions: [inspect_storage, finish]
max_parallel: 2
```

Through your MCP host, call `made_validate_ceremony_draft` with:

```json
{"definition_yaml": "<complete contents of two-checks.yaml>"}
```

Require `publishable: true`, review the analysis, then call
`made_publish_ceremony_definition` with the same exact `definition_yaml`.
The string is the file's contents, not its path. Start the published version
with `made_start_published_ceremony`:

```json
{
  "ceremony": "two_checks",
  "version": "1.0",
  "ceremony_id": "review-1",
  "actor_id": "review-host",
  "actor_kind": "service",
  "context": {"change_summary": "Review the proposed API and storage change"}
}
```

The counted-join publication fix in this source tree makes this same YAML
survive publication and SQLite reopen. Published v0.5.0 predates that fix:
validation alone there does not prove the definition can be published.

`change_summary` satisfies the definition's required input. The opening actor
is the actual caller, not necessarily a role at the table. Inspect the
instance, claim `inspect_api` as `API_REVIEWER` and `inspect_storage` as
`DATA_REVIEWER`, and use the [claim/work/complete loop](../runtime/README.md).
Only apply `finish` when the returned transition is enabled. Reusing
`review-1` is not a request to create an unrelated replacement session.

## Draft, analyze, publish

Use `made_design_ceremony` for a structured draft. Declare the objective,
participants, stages, outputs and any final human approval. The designer
produces linear stages; branching outcomes need explicit YAML. The
`roundtable_fixed_order` preset generates a sequential turn per participant
in declaration order, passing prior contributions forward. It does not
select speakers dynamically or aggregate their answers.

Validate the draft with `made_validate_ceremony_draft`; use
`made_explain_ceremony_draft` for a readable account of the same analysis.
A publishable draft is still a draft. `made_publish_ceremony_definition`
persists an immutable name/version/content identity. Change the definition
version when changing its content. `made_diff_ceremony_definitions` helps
review the change.

Start from the published identity for restart recovery. Supplying YAML
alone does not make that definition durable. Executable examples live in
[tests/e2e/ceremonies](../../tests/e2e/ceremonies); the reusable preset input
lives in [ceremony fragments](../../api/examples/ceremonies/fragments/README.md).

## Scheduling and guards

Sequential states expose the next work in order. Concurrent states expose
claimable work under a declared `max_parallel` and a runtime ceiling; effective
capacity is the lower limit. A host schedules the workers and claims each
step before running it. Inspect ready steps and enabled transitions after
every mutation instead of inferring them from an earlier snapshot.

Choose a join that matches the source state's work: `any_step_completed` permits an accepted
completion, while `steps_completed:n` requires the declared count. They are
scoped to the state being left. Required step-status guards can still keep a
transition blocked after a join is satisfied.

`all_steps_completed` is a global guard over the definition. On a transition
before later-state steps can run, it can wait on work that cannot become
eligible. Analysis now returns an advisory warning for this shape; it does
not change execution semantics or rewrite the guard. Prefer the appropriate
source-state join for concurrent work.

Human approval is a separate recorded decision. A design containing a human
guard does not grant the approval. A deferral preserves a statement, reason
and conditions for reconsideration and leaves the guard unsatisfied.

## Execution loops

| Mechanism | Why work runs again | Bound |
|:--|:--|:--|
| Retry attempt | A technical execution failed or a lease was replaced | Step retry policy and claim rules |
| Step repeat iteration | Successful output has not met a declared stop condition | `max_iterations`, 1–1000 |
| State repeat iteration | All work in a state succeeded but its stop condition is false | State `repeat.max_iterations`, 1–1000 |
| State visit | A transition enters a state again | Explicit transition traversal budget for cycles |

Repeat compares a top-level output field with a declared JSON value using
exact equality. Reaching a repeat limit does not silently restart or satisfy the stop
condition. A one-shot runner reports exhaustion; a definition can declare a
specific `step_repeat_exhausted:<step>` exit without waiving other guards. Transition budgets bound graph
cycles independently. See [runtime coordinates](../runtime/README.md) for
the unreleased state-visit contract.

A state-level `repeat` reruns all of its steps under a new `state_iteration`
once its work has finished and its condition is false. It is not a
self-transition. Its `until` names a step in that state, a top-level output
field and exact JSON equality. No live lease crosses the iteration boundary;
step iteration and attempt restart, while prior results remain in history.

`output_field:<step>:<field>=<json>` guards compare the current state
iteration's successful output. Missing output is false; malformed references
are rejected. There is no general expression evaluator or nested-path query.

The two repeat shapes differ only in where they are attached and whether
`until` names a source step:

```yaml
# On a step:
repeat:
  max_iterations: 3
  until:
    output_field: ready
    equals: true
```

```yaml
# On a state; check must be a step in that state:
repeat:
  max_iterations: 4
  until:
    step: check
    output_field: approved
    equals: true
```

Guard declarations name the exact check expression:

```yaml
guards:
  ready:
    type: automated
    check: 'output_field:check:approved=true'
  exhausted:
    type: automated
    check: 'step_repeat_exhausted:check'
```

These are syntax fragments, not complete definitions. Attach guard names to
the intended transition. The exhaustion guard applies only to the named
step's exhausted repeat; it does not waive another repeat, live lease,
human approval, open intervention or unmet required completion guard.

## Roles, context and output

A step can select eligible roles from declared context. Eligibility is
resolved for the accepted claim; later context edits must not silently
change the role that performed that work. Declared output-to-context writes
commit with the accepted result, under the engine's validation rules.
A refused completion contributes neither output nor context.

For a dynamic speaker, a step declares both a context selector and an allow-list:

```yaml
# Fields on a step whose id is speak, in a declared state.
role_from: context.next_role
allowed_roles: [AUTHOR, REVIEWER]
context_writes:
  next_role: selected_role
```

`next_role` is a top-level context key; it must contain an allowed role id.
Both roles must be declared and allowed to execute `speak`. The selected role
is sealed at claim time. `context_writes` maps destination context keys to
top-level output fields: the accepted result must contain `selected_role`,
whose value is copied into `next_role`. Missing fields fail rather than
partially applying the patch. There is no `context.` or `output.` prefix in
that mapping, and no nested-path language.

Root-level bounds use these fields:

```yaml
max_parallel: 3
max_transitions: 12
max_bounces: 3
```

`max_parallel` accepts 1–8 (default 3), capped again by the runtime ceiling.
`max_transitions` bounds all transitions for the session; `max_bounces` bounds
traversals of each exact declared `(from, to, trigger)` edge. The latter two
are positive integers when provided. A cyclic graph needs an explicit bound;
state repetition uses its separate iteration budget instead of consuming
transition counts.

Use structured output and evidence references that a consumer can inspect.
An output contract constrains shape; it does not prove an external claim is
true. Council output JSON Schema examples are documented
[here](../../api/examples/output-contracts/README.md).

## Boundaries and roadmap

Current primitives are building blocks. Automatic parallel drivers,
automatic agent spawning, general aggregation, complete group-chat/speaker
selection and the full orchestration pattern catalogue are not implemented
by declaring concurrency. Embedded council execution and configuration are
also not exposed. The earlier roadmap labels B3–B6, C1/C3, full D1–D5 and F5
remain future work; their
[historical plan](../history/pre-rebuild-2026-09-18/docs/orchestration-patterns-plan.md)
is context, not a current API promise.
