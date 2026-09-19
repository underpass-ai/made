# Author a ceremony

A ceremony definition is a versioned state machine with roles, steps,
transitions and guards. A session is one execution of that definition with
its own context and event stream. Define the decision or artifact first,
then the work that must produce it.

Use the [business vocabulary](business-lexicon.md) to distinguish the
Integrator's responsibility from contributors, reviewers, the host and engine.

## A complete definition

Save this as [two-checks.yaml](examples/two-checks.yaml). Two independent
inspections become claimable in `CHECKING`; the join counts completed work
in that source state. `max_parallel` permits the `run_ceremony` driver to claim
and execute both handlers concurrently. The handler names identify host
integrations, not bundled workers or automatically created agents.

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

The counted-join publication fix in 0.6.0 makes this same YAML
survive publication and SQLite reopen. Published v0.5.0 predates that fix:
validation alone there does not prove the definition can be published.

`change_summary` satisfies the definition's required input. The opening actor
is caller-declared provenance, not necessarily a role at the table. On a
protected surface the authenticated principal is separate; actor-shaped fields
cannot replace that principal or widen its grant. Inspect the instance, claim
`inspect_api` as `API_REVIEWER` and `inspect_storage` as `DATA_REVIEWER`, and
use the [claim/work/complete loop](../runtime/README.md).
Only apply `finish` when the returned transition is enabled. Reusing
`review-1` is not a request to create an unrelated replacement session.

### Integrator, implementers, independent reviewer and human guard

The executable [integrator delivery example](examples/integrator-delivery.yaml)
keeps four responsibilities separate: two implementers produce the inputs,
`INTEGRATOR` combines them, `INDEPENDENT_REVIEWER` reviews that combined result,
and `HUMAN_APPROVER` alone may perform `approve_delivery`. Validate and publish
the example with the same calls above, then drive it through the claim/work/
complete loop. The `implementations_complete`, `integration_complete` and
`independent_review_complete` guards must be satisfied before their transitions;
`human_approved` is a `type: human` guard and cannot be replaced by an agent
claim or a completed implementation.

The negative authorization check is intentional: `INTEGRATOR` is allowed
`integrate_delivery` and `integration_ready`, but not `approve_delivery`.
Changing only the role name to `INTEGRATOR` does not add authority, bypass the
independent review, or satisfy the human guard. The engine enforces the
declared role actions and guards; the host still supplies the actual handlers
and evidence.

Current support ends at declared roles, claims, completion records, guards and
host-provided execution. The live agent roster/activity surface described by
issues #190 and #191, and host delivery/acknowledgement for interventions in
#192, are future capabilities; this example does not imply any of them.

## Draft, analyze, publish

Use `made_design_ceremony` for a structured draft. Declare the objective,
participants, stages, outputs and any final human approval. The designer
produces linear stages; branching outcomes need explicit YAML. The
`roundtable_fixed_order` preset generates a sequential turn per participant
in declaration order, passing prior contributions forward. It does not
select speakers dynamically or aggregate their answers.

Compose coordination shapes per stage with a `pattern` object. Every pattern
declares its roles and instructions; bounded patterns also declare
`max_iterations` and a `fallback_role_id`. `broadcast_collect`, `group_chat`
and `magentic` additionally declare `manager_role_id`.

```json
{
  "id": "analysis",
  "pattern": {
    "kind": "broadcast_collect",
    "roles": ["OPERATIONS", "SECURITY", "PRODUCT"],
    "manager_role_id": "INCIDENT_LEAD",
    "instructions": "Analyze the same incident timeline independently."
  }
}
```

Available kinds are `sequential`, `concurrent`, `broadcast_collect`,
`group_chat`, `maker_checker`, `handoff` and `magentic`. The designer expands
them into ordinary states, steps, guards and transitions. Its `x-pattern`
state annotation exists for renderers and reports and never changes runtime
semantics. See the executable [incident review](../../tests/e2e/ceremonies/incident_review.yaml),
[concurrent review](../../tests/e2e/ceremonies/concurrent-review.yaml), and
[fragment catalog](../../api/examples/ceremonies/fragments/README.md).

Pattern stages that branch on, or write context from, a deliberation result
declare the required top-level JSON fields in
`config.project_winner_fields`. The deliberating handler copies only those
fields into the step output while retaining `winner_content`, `task_id`,
`winner_proposal_id` and `candidates_total`. Without this opt-in, its output
keeps the historical four-field shape. The configured names must be unique,
non-empty and must not collide with those metadata names; invalid JSON,
non-object JSON and missing declared fields fail the step.

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
capacity is the lower limit. The `run_ceremony` driver claims ready siblings
before it invokes their host-provided handlers and drains every accepted claim
before evaluating the next transition. A host may instead use the claim and
completion APIs to schedule its own workers. Inspect ready steps and enabled
transitions after every mutation instead of inferring them from an earlier
snapshot.

Choose a join that matches the source state's work: `any_step_completed` permits an accepted
completion, while `steps_completed:n` requires the declared count. They are
scoped to the state being left. Required step-status guards can still keep a
transition blocked after a join is satisfied.

`all_steps_completed` is a global guard over the definition. On a transition
before later-state steps can run, it can wait on work that cannot become
eligible. Analysis now returns an advisory warning for this shape; it does
not change execution semantics or rewrite the guard. Prefer the appropriate
source-state join for concurrent work.

## Spawn child ceremonies

A step can open published ceremonies as durable children. Publish every child
definition first, then declare its exact name and version. `inputs` maps each
required child input to a key in the parent's sealed context:

```yaml
steps:
  - id: delegate_reviews
    state: REVIEWING
    handler: child_orchestration
    spawn:
      children:
        - ceremony: specialist_review
          version: "1.0"
          inputs:
            brief: review_brief
        - ceremony: specialist_review
          version: "1.0"
          inputs:
            brief: review_brief
      max_children: 2
      max_depth: 3
guards:
  enough_reviews:
    type: automated
    check: "children_completed:delegate_reviews:any"
```

The engine resolves every publication, projects and validates every input,
captures memory recollection and seals the complete plan before opening the
first child. Child and group ids derive from the parent, step, visit,
iterations and declaration position. Replaying the same plan therefore
verifies the same streams instead of creating replacements. `max_children`
bounds the declared width. `max_depth` is also constrained by the parent's
remaining budget, which decreases at every generation.

The spawn step completes after all planned child streams have been opened or
verified. That does not mean the children have finished. A later transition
uses `children_completed:<step>:all`, `:any`, or `:quorum:<n>`. A completion
counts only after MADE locates the named `CeremonyCompleted` record in an
intact child journal, verifies its hash and opening against the sealed plan,
and records the acceptance in the parent. A completed child may acquire later
facts when it is itself a parent; its original terminal record remains the
stable completion locator.

`made_run_ceremony_step` and `made_run_ceremony` route spawn steps through this
protocol, including steps in concurrent states. They do not invoke the step's
ordinary handler for a spawn. MCP hosts call `made_prepare_ceremony_children`,
and gRPC hosts call `PrepareCeremonyChildren`, with the actor plus optional
lease owner, idempotency key and TTL; either operation claims and executes the
spawn. The lower-level Rust facade `prepare_children` instead receives an
already accepted claim fence. Recovery uses `made_recover_ceremony_children`;
broker notifications only wake that work, while the durable global event
cursor decides what remains pending.

## Aggregate concurrent outputs

The first step in the sequential state after a concurrent state may declare an
`aggregate` policy. That transition must wait for every sibling in the source
state, and the aggregate step must set `see_prior: true`. These constraints
make the input set complete and unambiguous before execution starts. Analysis
accepts either one successful status guard per sibling or a state-scoped
`steps_completed:n` whose count equals the source state's width. It rejects an
early `any` or smaller counted join, more than one predecessor, a later step in
the destination state, or a reused state visit. The global
`all_steps_completed` guard is not treated as a source-scoped join.

`synthesize` invokes the step's existing handler or council. Its brief receives
one final output per sibling, in sibling declaration order:

```yaml
steps:
  - id: synthesize
    state: SYNTHESIS
    handler: editorial_council
    config:
      see_prior: true
    aggregate:
      strategy: synthesize
```

`vote` makes no handler or model call. It compares one declared top-level
output field with exact JSON equality and writes the winning value under that
same field:

```yaml
steps:
  - id: select
    state: DECISION
    handler: host_callback
    config:
      see_prior: true
    aggregate:
      strategy: vote
      output_field: recommendation
```

A value wins only with more than half of all sibling votes. JSON `null` is a
present value. A missing sibling, a missing field, or a tie/plurality without a
strict majority fails the claimed step explicitly. Aggregation reads only the
immediately preceding state visit and its final state iteration, so a cycle or
reopened ceremony cannot mix older outputs into the result.

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
the 0.6.0 state-visit contract.

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

The source tree's Unreleased surface includes the bounded concurrent driver,
typed aggregation, runtime observability, the stage-pattern catalogue described
above, embedded council configuration and execution, durable child ceremonies,
and resumable ceremony progress. MADE invokes configured handlers and councils;
it does not create host subagents. A host can fan work out to its own subagents
through those handlers or can drive the durable claim/complete protocol
directly. Child ceremonies create bounded, durable sessions; they do not create
operating-system processes, provision agents, or grant tools and credentials.

The published 0.6.0 package predates those Unreleased additions. The earlier
roadmap remains useful for features outside this contract; its
[historical plan](../history/pre-rebuild-2026-09-18/docs/orchestration-patterns-plan.md)
is context, not a current API promise.
