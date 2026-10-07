---
name: run-ceremony
description: Execute or resume a MADE ceremony when the user asks to run a structured procedure, using real host work and recorded decisions.
---

# Run a ceremony

Decision rules. The running server's `made_get_help` (`audience: "agent"`)
carries the delegated-host sequence, the integrator loop, the intervention
routes and the declared limits; `tools/list` carries the exact schemas.
Read `made_discover_capabilities` first: it says which tools this session
lists (`tool_profile`), where a person's approval is accepted
(`human_approval`) and which listed tools this host holds no grant for
(`authorization.listed_tools_without_grant`).

## When a tool is refused for lack of a grant

- A `refused` answer saying the trusted host `holds no live grant` names
  the action. It is the authority boundary, not a defect: tell the person
  the action and that `scripts/made-grant.sh --profile core` (the ordinary
  route) or `--actions <name>` in their own terminal grants it, then call
  again. The running session sees a new grant on its next call.
- Never grant yourself: do not run the script, pipe an answer into it, or
  ask for a wider tool profile in order to call
  `made_issue_authorization_grant` on your own behalf.

## Start

- Publish the reviewed definition and start by its published name and
  version with `made_start_published_ceremony`, so the session survives a
  restart. Keep caller-chosen ceremony ids stable. Declare the real actor:
  an agent acting for a person is still `agent`.
- After every action read the returned instance. Work only on
  `claimable_step_ids`; apply only enabled transitions. A recurring step id
  is told apart by `state_visit`, `state_iteration`, `iteration`, `attempt`.

## Do the work

- One owner per step. Delegated work is `made_claim_ceremony_step`, the real
  work with the host's own tools, then `made_complete_ceremony_step` with the
  observed status, structured output and the claim's `claim_fence` unchanged.
  A claim performs nothing; a no-op handler proves wiring only; never report
  unperformed work as completed.
- Renew with `made_renew_ceremony_step_lease` before expiry while the host
  still owns authorized work. A replay returns the original receipt, not new
  authority.
- Concurrent siblings: claim a bounded set with distinct idempotency keys,
  one worker each, completions in any order. If the host cannot fan out,
  drive serially and say so.
- Stop and surface the failure when `isError` is true or `completed` is
  false. Retry only after its cause changed.

## When a person must decide

- With `human_approval.source: terminal`, `made_approve_ceremony_guard` is
  refused on this session. Present the concrete decision and tell the person
  to run `scripts/made-approve.sh --ceremony <id> --guard <name> --role
  <role>` in their own terminal, then read the instance again. Do not drive a
  pseudo-terminal, pipe an answer or set an environment variable to answer
  for them.
- With `human_approval.source: host`, record `role_kind: human` only after
  the person stated the decision in the current conversation. Silence, a
  prior general instruction or an agent result never satisfies a human guard.
- A deferred decision is `made_defer_ceremony_guard` with the person's
  statement, the reason and concrete `reconsider_when`; the session stays
  paused. Deferral does not approve.

## Recover

- After context loss, list instances before starting anything. A
  non-rehydratable snapshot is evidence of a session, not a runnable one.
- A lost claim receipt: establish which claim did the work; never borrow the
  current worker's identity; let a lease expire before a new claim.
- A paused session whose definition turned out wrong: `made_plan_ceremony_successor`
  (seals nothing), then `made_start_ceremony_successor` with your own
  `plan_id`. The old session can then only be cancelled.
- Resuming replays no side effects and approves nothing.

## Interventions, the loop and the record

- Interventions, agent status and the integrator loop are in capability
  groups the `core` profile hides. If the task needs them, say which group to
  add (`MADE_MCP_TOOL_PROFILE=core+ceremony_participation+integrator_loop`)
  and follow `made_get_help` once they are listed. A delivery label a
  participant wrote about itself is never evidence that anything arrived.
- Read the record with `made_read_ceremony_events` (page by `next_version`),
  `made_get_ceremony_transcript` and `made_verify_ceremony_journal`. An intact
  journal proves order and seals, not the truth of external outputs.
- `made_generate_ceremony_report` returns Markdown with `persisted: false`;
  save it only where the user asked. A signed copy for someone without the
  store is `scripts/made-export-evidence.sh --ceremony <id> --out <file>`.
