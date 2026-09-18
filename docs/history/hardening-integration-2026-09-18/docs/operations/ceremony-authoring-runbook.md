# Ceremony authoring runbook — building multi-agent meetings that actually work

*Status: the original mechanics were verified end-to-end on 2026-07-03 against
a live installation (chart + `RunCeremony`), including a 7-participant,
31-LLM-call ceremony. Bounded repetition (§6) was verified on 2026-09-02 by
the core, adapter, application, MCP and contract gates.*

The architecture docs explain *how the engine deliberates*
([made-architecture-and-differentiation.md](../made-architecture-and-differentiation.md));
this runbook is for the operator **writing a ceremony YAML**: the schema keys
that matter, the mechanics they trigger, the silent no-ops, how to size
timeouts, and how to verify what actually happened.

## 0. Where a first draft comes from

Nothing below requires starting from a blank file. `made_design_ceremony`
takes an objective, the participants and the ordered stages and renders a
ceremony YAML with the states, completion guards, role actions,
timeouts and retry policy already consistent — then analyses it, exactly as
`made_validate_ceremony_draft` would. It publishes nothing and starts
nothing, and both editions serve it: the MCP tool on either backend, and the
`DesignCeremony` RPC for a client that speaks gRPC directly.

The designer also accepts a grouped stage. Its children become separate steps
in one state, while the outer stage id names that state:

```json
{
  "max_parallel": 3,
  "stages": [{
    "id": "review",
    "group": {
      "execution": "concurrent",
      "steps": [
        {"id":"security_review","owner_role_id":"SECURITY","instructions":"Review security."},
        {"id":"operations_review","owner_role_id":"OPERATIONS","instructions":"Review operations."}
      ],
      "join": {"condition":"steps_completed", "count":2}
    }
  }]
}
```

Each concurrent child must have a distinct role owner. Join conditions are
`all_steps_completed`, `any_step_completed`, or `steps_completed` with a
positive `count`. The first child's owner owns the generated outgoing
transition. `max_parallel` belongs to the definition (default 3, range 1–8);
the server applies the lower of it and `MADE_MAX_PARALLEL` (default 8) without
rewriting the stored definition. Live clients should fan out over every id in
`claimable_step_ids`, complete claims in any order, then transition. A live
lease always blocks the transition, including an early `any_step_completed`
join; an expired lease does not.

`all_steps_completed` checks every step in the ceremony, including future
states. It is suitable after all required work is already reachable. On a
review-to-synthesis transition it would also wait for synthesis, which cannot
start yet. Use `steps_completed` with the number of source-state steps, or
explicit `step_status:<step>:COMPLETED` guards for the required source steps.
`any_step_completed` and `steps_completed` are scoped to the source state.
Draft analysis warns when a global completion barrier precedes downstream
steps that cannot be reached without crossing such a barrier. This is an
advisory graph check: other guards can still block execution, and cycles or
detours that allow the work to happen first remain publishable.

Definitions with a cycle must declare at least one positive transition budget:

```yaml
max_transitions: 20  # total applied transitions in one instance
max_bounces: 3       # applications of each exact from/trigger/to edge
```

Either field is enough to make the graph bounded; when both are present, both
limits apply. Counts come from sealed `TransitionApplied` history, so replay,
snapshots and SQLite reopen keep the same remaining budget. Starting another
state iteration through `repeat` consumes no transition. A component avoids the
no-human-control warning when an outgoing transition from any member state,
including an exit edge, requires a human approval guard.

Every new transition opens a durable `state_visit` and resets the destination
steps to pending at state iteration 1, step iteration 1 and attempt 1. Earlier
executed records and outputs stay in history with their original coordinates.
Within-state `repeat` advances `state_iteration` inside the same visit. The
initial visit is 1; transitions advance it, including terminal entry. Historical
transition events without a sealed destination reset retain their previous fold;
they are not retroactively interpreted as fresh work. See
[ADR-018](../adr/018-durable-state-visits.md).

What it will not do is the rest of the part this runbook is about. Its outer
topology remains ordered, so branching, alternative terminal
outcomes and anything a stage needs that the intent contract cannot say are
written by hand, and every key below is what you are writing when you do.

## 1. The step config keys that drive everything

```yaml
steps:
  - id: technical_review
    state: TECHNICAL_REVIEW
    handler: appeal_engineers        # = the agent SPECIALTY (registry key)
    config:
      agent_kind: vllm               # noop | vllm (feature/agent_kinds-gated)
      provider.model: "role:coding"  # exact served-model name OR role alias
      provider.max_tokens: 640
      provider.timeout_secs: 240     # per LLM CALL, not per step
      see_prior: true                # prior steps' outputs enter the prompt
      rounds: 3                      # adversarial peer-review rounds
      num_agents: 2                  # council size
      prompt: "..."
      output_contract:               # optional deterministic policy gate (#118)
        contract_id: my-contract     # unknown keys inside this block are rejected
        format: json_object
        required_fields: [claims, decision]
        allowed_values:
          decision: [accept, reject, request_changes, request_more_evidence]
        json_schema: { ... }         # optional embedded JSON Schema
        evidence:                    # optional grounding rule (#119): every claim
          claims_field: claims       # object must cite refs that exist in the
          refs_field: evidence_refs  # allowed set...
          allowed_refs_from_context: evidence_pack   # ...resolved per-run from
                                     # the RunCeremony context (or a static
                                     # allowed_refs list)
```

With an `output_contract` present, proposals that fail the gate are rejected
deterministically (`NoValidProposal{contract_id}` fails the deliberation);
with an `evidence` block, the `claims-evidence-grounded` validator enforces
that no unsupported claim reaches the step's winning contribution.

Facts that are easy to get wrong:

- **Steps in the same state run in YAML declaration order.** The step `id`
  is an identity and lookup key; it does not determine execution priority.
  Put dependent steps later in the list and use `see_prior: true` when they
  must receive earlier contributions.
- **`handler` is the specialty.** Agents and councils persist in the registry
  **by specialty id** for the lifetime of the pod (in-memory persistence).
  Re-running a ceremony with the same handler but a different `agent_kind`
  reuses the OLD agents — restart the pod or pick a fresh handler name.
- **`provider.*` lives in the step config**, not at ceremony level. Each step
  can point at a different model, size and timeout.
- **`see_prior: true` is your reply mechanism across steps** — later steps
  receive earlier outputs as context. Replies *within* a step are `rounds`.

## 2. `rounds` — real replies, with a silent no-op

`rounds: N` runs the engine's adversarial peer review: **for each round, each
agent critiques its neighbour's proposal ((i+1) mod N, deterministic circular
rotation) and the neighbour's proposal is replaced by the revision.**

- **`rounds` with `num_agents: 1` is a silent no-op** (`deliberate.rs`:
  `if rounds == 0 || agents.len() < 2 { return }`). No error, no warning —
  the step just runs a single draft. If you want back-and-forth, you need a
  council of at least 2.
- LLM call count per step: `num_agents` drafts + `rounds × num_agents × 2`
  (each review = one critique call + one revise call). A 2-agent, 3-round
  council = **14 sequential calls**. Budget your step timeout accordingly:
  `step timeout ≥ calls × p95-per-call-latency`, and remember
  `provider.timeout_secs` bounds each call, not the step.
- Every review emits a `"peer critique and revision"` log/span event with
  `round` and `reviewer` — so "who spoke how many times" is a Loki/Tempo
  query, not archaeology.

## 3. Role aliases mix models *per call*

If the provider behind `MADE_VLLM_ENDPOINT` is a routing gateway that
resolves aliases (e.g. `role:coding` → round-robin over the models carrying
that role), then **every individual call — draft, critique, revision —
resolves independently**. In a 3-round council the model that critiques a
proposal is routinely not the model that drafted it. This is a feature (true
model heterogeneity inside one council) and a caveat (MADE's
logs record the agent id, not which physical model served each call — if you
need that attribution, log the resolved model in your gateway).

Exact served-model names (`provider.model: "phi4-mini-aws"`) pin a step to
one model; aliases (`role:general`) opt into the mix.

## 4. What multi-round councils actually do (verified, and humbling)

From the verified 31-call run — two councils asked *"why did your previous
investigation fail?"* with 3 peer-review rounds each:

- **Rounds amplify fluency, not grounding.** Without a validator that checks
  claims against the case evidence, each critique round acted as "elaborate
  further": by round 3 both councils had drifted into impressive,
  well-structured, largely fabricated process machinery, far from the
  question.
- **Narrow roles beat big models.** A 3B model given a terse
  "mark each claim GROUNDED or SPECULATION" task produced the most
  evidence-faithful output of the ceremony, outperforming 24-30B models
  deliberating freely.
- Practical guidance: pair every free-deliberating council with (a) a
  grounding step whose only job is separating evidence from speculation, and
  (b) a judge prompt that must cite which claims it relied on. Keep council
  prompts adversarial about *grounding* ("attack claims not supported by the
  stated facts"), not just adversarial in tone.

## 5. Sizing and invocation checklist

1. **Lint before running**: `CeremonyDefinitionYaml::parse_path` (a five-line
   test in `crates/made-adapters/tests/`) catches schema errors in
   milliseconds instead of after a cluster round-trip.
2. **Count your calls**: sum over steps of
   `num_agents + rounds × num_agents × 2`. Multiply by expected per-call
   latency; set `timeouts.step_default` above the slowest step, and your
   gRPC client's timeout above the whole sum (`RunCeremony` blocks until the
   terminal state).
3. **Thinking models**: budget `provider.max_tokens` for reasoning + answer
   (a thinking model can spend its entire budget inside `<think>` and return
   an empty answer).
4. **Guards**: `step_status:<step_id>:COMPLETED` chained through the FSM
   transitions is the standard linear-meeting pattern.
5. **Verify what happened, not what you asked for**: count
   `proposal drafted` and `peer critique and revision` events per agent
   (Loki), and read the deliberation spans (Tempo) — see the
   [observability runbook](./observability-runbook.md). If `rounds` was
   silently no-opped (§2), this is where you notice.

## 6. Bounded `repeat` — successful iteration until a condition

Use `repeat` when a step can succeed operationally but its structured result
says the work is not finished yet:

```yaml
steps:
  - id: refresh_and_validate
    state: COORDINATING
    handler: evidence_reviewer
    repeat:
      max_iterations: 4
      until:
        output_field: ready
        equals: true
    config:
      see_prior: true
      prompt: "Refresh the evidence and return a top-level ready boolean."
```

`output_field` addresses one top-level field in the successful step output;
`equals` is compared using exact JSON equality, so `true` is different from
`"true"`. A missing field does not satisfy the condition. `max_iterations` is
mandatory, must be between 1 and 1000, and is the safety bound that prevents a
ceremony from spinning forever.

A **repeat iteration** follows a successful result whose condition is false.
Its output is retained in durable history and included in the transcript for
the next iteration. A **retry attempt** re-drives failed or expired work inside
the same semantic iteration. Runtime views expose both `iteration` and
`attempt`, plus `repeat_condition_satisfied` and `repeat_limit_reached`.

When the condition becomes true, normal guards and transitions may fire. When
the final permitted iteration still returns false, ordinary transitions remain
blocked; one-shot execution records the `repeat_limit` outcome metric and
returns a repeat-limit error instead of silently completing or looping. An
explicit `step_repeat_exhausted:<step>` guard may route that bounded outcome:

```yaml
transitions:
  - from: COORDINATING
    to: NEEDS_ATTENTION
    trigger: escalate_refresh
    guards:
      - refresh_completed
      - refresh_exhausted
      - facilitator_approved
guards:
  refresh_completed:
    type: automated
    check: "step_status:refresh_and_validate:COMPLETED"
  refresh_exhausted:
    type: automated
    check: "step_repeat_exhausted:refresh_and_validate"
  facilitator_approved:
    type: human
    check: manual_approval
```

Required guards are a conjunction. Here exhaustion waives only
`refresh_and_validate`'s unmet repeat condition on this transition; it does not
waive `facilitator_approved`, another repeating step, a required completion or
join guard that is blocked by an active lease, or an open intervention.

Use `output_field:<step>:<field>=<json>` to guard a transition with an exact
top-level output value:

```yaml
guards:
  prior_result_selected:
    type: automated
    check: 'output_field:produce_candidate:decision={"selected":true}'
```

The step must be declared, but it may belong to an earlier state. MADE reads
that step's current record and requires a successful status; it never falls
back to an older successful output after the current record fails or a repeat
opens a new iteration. A missing field is false, and JSON types remain exact.
Fields may contain `=` and JSON strings may contain `=`; the parser identifies
the unique split whose suffix is valid JSON.

`made_design_ceremony` exposes the same conditions as `stages[].exit_guards`:

```json
{
  "exit_guards": [
    {
      "kind": "output_field",
      "step": "produce_candidate",
      "output_field": "decision",
      "equals": {"selected": true}
    },
    {"kind": "step_repeat_exhausted", "step": "refresh_and_validate"}
  ]
}
```

These guards are added to the generated completion guard, group join guard
when present, and final human approval. `equals` is required; explicit `null`
means JSON null rather than omission.

### Repeat a whole state

Put `repeat` on a state when every step must run again as one bounded unit:

```yaml
states:
  - id: REVIEW
    initial: true
    terminal: false
    execution: concurrent
    repeat:
      max_iterations: 4
      until:
        step: check
        output_field: approved
        equals: true
```

MADE waits for every state step and each step's own repeat policy before it
tests the state condition. A false result resets every step to pending under
the next `state_iteration`; `iteration` and `attempt` restart at one. Prior
records and transcript contributions remain available with both coordinates.
An early concurrent join does not skip the rest of a repeated state's work.
If the final permitted state iteration is still false, the instance reports
`state_repeat_limit_reached` and one-shot execution records the distinct
`state_repeat_limit` outcome.

The design tool expresses the same policy on a grouped stage. The `step` named
by `until` must be one of that group's children:

```json
{
  "id": "review",
  "group": {
    "execution": "concurrent",
    "steps": [
      {"id":"draft","owner_role_id":"AUTHOR","instructions":"Draft."},
      {"id":"check","owner_role_id":"REVIEWER","instructions":"Return approved."}
    ],
    "join": {"condition":"steps_completed", "count":2},
    "repeat": {
      "max_iterations": 4,
      "until": {"step":"check","output_field":"approved","equals":true}
    }
  }
}
```

State repeat is separate from leaf `repeat`, which still repeats one step.
All direct gRPC, MCP-over-gRPC, embedded MCP, and Rust facade reads expose the
same current state coordinate, per-step coordinate, run trace and transcript.

## 7. Route a step from context and write results back

Use `role_from` when the participant for a leaf step is selected by a prior
result rather than fixed when the definition is written. The selector is one
top-level context key, and `allowed_roles` is the complete set it may name:

```yaml
steps:
  - id: choose_reviewer
    state: DRAFT
    handler: reviewer_router
    context_writes:
      next_role: selected_reviewer
  - id: review
    state: REVIEW
    handler: evidence_reviewer
    role_from: context.next_role
    allowed_roles: [LEGAL_REVIEWER, SECURITY_REVIEWER]
```

`choose_reviewer` must complete with a top-level `selected_reviewer` output.
MADE appends its `StepCompleted` and `ContextWritten` facts together, in that
order. Every declared source must be present before either fact is accepted;
an explicit JSON `null` is present and is copied. If two completed steps write
the same destination, the last sealed `ContextWritten` in stream order wins.

When `review` is claimed, MADE requires `context.next_role` to be a string in
the allow-list and requires that role to be authorised for the step. The claim
seals the chosen role. Changing context later cannot change `finished_by` for
that attempt. In a concurrent state, another step in the same state iteration
cannot claim the same sealed role; an expired lease keeps its reservation until
that step is validly reclaimed or the state iteration resets.

Static steps obey the same role reservation rule. New concurrent claims using
an alternate authorised role seal that role explicitly. Unmarked historical
static records reserve their canonical owner without changing stored events
or replayed record shapes.

Role-less MCP and gRPC claim/run requests resolve the role inside each
optimistic decision, including retries after another step changes context.
Rust facade callers using `StartCeremonyStepInput::new` or
`RunCeremonyStepInput::new` retain explicit role selection and mismatch refusal.
Use `.with_automatic_role_resolution()` on either input to select from current
context instead. In that mode, the input's `role_id()` remains its compatibility
anchor; the accepted role comes from the sealed claim and execution record.

The design tool uses the same fields on a leaf stage or on a child of a grouped
stage. They cannot be placed on the group container. Its `owner_role_id` still
owns the generated transition; every `allowed_roles` entry receives permission
to execute the leaf. Omitted fields keep the static role and legacy YAML shape.

## 8. Dynamic participant interventions

An embedded incremental ceremony can accept new agenda items after it starts;
the YAML remains the stable frame while the running `CeremonyInstance` owns
the ordered conversation. Declare capabilities on roles rather than inventing
placeholder steps for every possible question:

```yaml
roles:
  - id: ENGINEER
    allowed_actions:
      - request_intervention
  - id: OBSERVER
    allowed_actions:
      - respond_to_intervention
  - id: DATABASE_SPECIALIST
    allowed_actions:
      - respond_to_intervention
  - id: QUEUE_SPECIALIST
    allowed_actions:
      - respond_to_intervention
```

The host opens an `opinion`, `investigation`, or `action` with
`made_request_ceremony_intervention`. With no `target_role_ids`, every role
with response capability may answer; a non-empty list scopes the request.
Each targeted role can answer once, and only the requesting role can close the
intervention. Relevant requests and accumulated responses are passed into
later deliberating step handlers as live participant language, so the meeting
can react without rewriting its definition.

Treat `action` as coordination, not authority. “Look at the queue” should be
implemented as read-only observation or peek, never consuming messages; any
external mutation still requires the host's permissions and the ceremony's
explicit human guards.

## 9. `memory_scope` — what earlier sessions decided

*Status: verified on 2026-09-17 by `made-app`'s start tests, `made-core`'s
rendering tests and the MCP parity session, which drives two sessions in one
scope on both backends and compares what the second one was told.*

A working session records what it decides — contributions, guard decisions,
how it ended, and the reasons between them — into memory under a **scope**. A
session that declares none gets `ceremony:{its own id}`, and that means **no
shared memory**: nobody else ever looks there, so nothing it writes reaches a
later session and nothing a later session decides reaches it.

Declaring a scope is one reserved key in the ceremony's start context:

```json
{
  "ceremony_id": "editorial-2026-09-17",
  "definition_yaml": "...",
  "actor_id": "operator-1",
  "actor_kind": "service",
  "context": { "memory_scope": "team:editorial" }
}
```

When the session opens, the engine reads that scope and seals what it was told
into the stream as `memory_recalled`, right after `ceremony_instance_started`.
It is then in every read of the session — `made_get_ceremony_instance`,
`made_read_ceremony_events`, the report's *What earlier sessions decided*
section, the facade's `CeremonySummary` — on both editions:

```json
"recollection": {
  "scope": "team:editorial",
  "truncated": false,
  "entries": [
    {
      "entry_id": "agenda:which-rollback:contribution:0",
      "kind": "decision",
      "summary": "Roll back rather than restart.",
      "from_ceremony_id": "editorial-2026-09-10",
      "observed_at": "2026-09-10T09:12:00Z"
    }
  ]
}
```

A session that was told nothing carries `"recollection": null`, which is what
every session that declares no scope carries and what a declared scope nobody
has written to carries too.

### The scope's grammar

`kind:name` — the kind lowercase ASCII letters, digits, `_` or `-`; the name
non-empty and free of control characters; at most 256 characters in all. The
kind is required and is what keeps two hosts that both keep memory — about
their cases, their tickets, their teams — from sharing one namespace by
accident. `ceremony:{id}` is the same grammar, which is why the default needs
no exception.

A `memory_scope` that is present and is **not** a usable scope refuses the
start. Falling back to the private default would hand you a session that
remembers alone while you believe it is sharing, which reads exactly like a
memory that lost the entries.

### What comes back, and how much

Decisions and constraints first, then observations and outcomes, each group in
the order memory returned them, up to **4096 bytes of summary**. The rendering
stops at the first entry that will not fit and says `"truncated": true`; an
entry larger than the whole budget is dropped rather than cut, because half a
decision is a sentence that says something else.

Memory is not the transaction. A memory backend that cannot be read costs the
session its recollection and nothing else: the start succeeds, a warning is
logged with the scope, and `recollection` is `null`.

### Declaring it in the definition

The engine reads the context, not the definition, so a definition does not have
to mention it. Declaring it anyway is how a reader of the YAML learns that this
ceremony is meant to share a memory:

```yaml
inputs:
  required:
    - brief
  optional:
    - memory_scope
```

`made_design_ceremony` carries the same names through its `optional_inputs`.
Note that inputs are not enforced at start today (issue #28), so this is
documentation for whoever reads the definition, and the value still travels in
the start context.

### What is not remembered

A step running is machinery and an agenda item is a question; neither is a
decision, an observation, a constraint or an outcome, and memory has no fifth
kind for the rest. Transcripts are not memory: the reference is remembered, the
narrative stays where it was produced.
