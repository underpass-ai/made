---
name: run-ceremony
description: Run a declarative MADE ceremony locally from YAML when the user asks to coordinate or execute a structured ceremony.
---

# Run a MADE ceremony

Use the tools exposed by the bundled MADE MCP server. Choose the
one-shot path only when the ceremony can run to completion without a later
human decision and the active host has configured real handlers for every
step. The bundled embedded default may use `NoopCeremonyStepHandler`; a
completed no-op proves engine wiring, not that search, scraping, modelling,
rendering, or artifact creation happened.

When the active plugin build or backend is uncertain, call
`made_discover_capabilities` before choosing a path. Use
`made_get_help` with `audience: agent` for the server's current preconditions,
authority boundaries, delegated-host sequence and error handling. Those tools
describe the running server; this skill supplies the deeper execution policy.

When the user asks to design or create a ceremony rather than run supplied
YAML, follow the `design-ceremony` skill and call `made_design_ceremony`
before using this execution workflow.

## Choose the execution owner

Use exactly one of these paths for each step:

- Server-owned handler: only call `made_run_ceremony` or
  `made_run_ceremony_step` after the active host confirms that the declared
  handler is backed by a real `CeremonyStepHandlerPort`. Inspect its returned
  output and evidence before advancing.
- Delegated host: call `made_claim_ceremony_step` for the exact
  `next_step_id`, perform the real work through the host's authorized worker
  and tools, then call `made_complete_ceremony_step` with the observable
  status and structured output, including artifact or evidence references.
  Refresh with `made_get_ceremony_instance` before applying an enabled
  transition.

A successful claim records a lease and performs no stage work. Never record
attempted, simulated, inaccessible, or empty work as completed evidence. These
tools coordinate existing host authority; they do not grant permission to use
an external system.

## One-shot ceremonies

1. Obtain or construct a valid ceremony YAML definition.
2. Keep the ceremony id stable when the user supplies one; otherwise let the
   engine generate it.
3. Put caller-provided data in the ceremony `context` object.
4. Call `made_run_ceremony` with `definition_yaml`, the optional context, and
   `actor_id` / `actor_kind` for whoever is running it. Declare what you
   actually are — an agent running this on someone's behalf is `agent`, not
   `human`. The engine records what you say and infers nothing, and every
   entry the run leaves behind names it.
5. Report the final state, completion status, and step results. Surface the
   Mermaid sequence when it materially helps explain the execution.

Do not claim that a ceremony completed if the tool returned `isError: true` or
`completed: false`. A bounded repeat that exhausts `max_iterations` returns an
explicit repeat-limit error; it must not be described as a completed step loop.

## Incremental ceremonies with human authorization

1. Call `made_start_ceremony` with the YAML, initial context, and
   `actor_id` / `actor_kind` for whoever is opening it.
   Keep its `ceremony_id` for every later call.
2. While `next_step_id` is present, choose the verified server-owned handler
   path or the delegated-host claim/work/complete path above for that exact
   step, declaring `actor_kind`. Re-read the returned instance after every
   action. The same `next_step_id` can legitimately appear again with a higher
   `iteration` when its successful output has not met a declared `repeat`
   condition. Inspect `repeat_condition_satisfied` and
   `repeat_limit_reached`; never confuse the semantic iteration with the
   technical retry `attempt`.
3. When `waiting_for_human` contains guard names, pause the ceremony and ask
   the user to authorize or reject the concrete decision. Explain what
   transition the approval would enable.
4. Never infer approval from silence, prior instructions, an agent result, or
   the fact that approval seems operationally sensible. Call
   `made_approve_ceremony_guard` only after explicit human authorization in
   the current conversation. Its `role_kind` is `human` only when a person
   authorized it in this conversation; if you are recording your own decision,
   say `agent`. The engine writes down what you declare and checks nothing, so
   this field is where a receipt becomes true or false.
5. Call `made_apply_ceremony_transition`, declaring `actor_kind`, only when
   the returned transition reports `enabled: true`.
6. Repeat from step 2 until `completed: true`. Use
   `made_get_ceremony_instance` whenever state must be refreshed without a
   mutation.

If the user is uncertain or defers approval, call
`made_defer_ceremony_guard`. Preserve their own words in `statement`, state
why the decision remains unclear in `reason`, record concrete
`reconsider_when` conditions, and declare `role_kind` as above. Leave the persistent ceremony paused and report
its `ceremony_id`, current state, and blocking guard. A deferral never satisfies
the guard. Do not convert a refusal into a tool error or silently choose another
transition.

## Resuming after host context loss

When the host restarted or no longer has the ceremony id:

1. Call `made_list_ceremony_instances` before starting a replacement.
2. If exactly one incomplete instance matches the incident context, refresh it
   with `made_get_ceremony_instance` and continue from the returned
   `next_step_id`, guard, or open intervention.
3. If several incomplete instances could match, show their ids, states, and
   open interventions and ask the user which meeting to resume.
4. If none exists, state that the prior instance is not recoverable from the
   active backend. Start a reconstructed successor only with a new stable id
   and explicit recovery provenance; never call it the original meeting.

Discovery and refresh are read-only. They never approve a guard, close an
intervention, or replay an operational action. The bundled launcher keeps
ceremony state in a SQLite file, so a restarted MCP process finds its
instances again — but only those started from a *published* definition come
back whole. An instance started from supplied YAML is listed with
`"rehydratable": false` and cannot be resumed: treat it as evidence that the
meeting happened, not as a meeting you can continue.

## Dynamic participant interventions

When a participant asks the meeting for an opinion or asks a role to inspect
something, keep the ceremony instance active and use
`made_request_ceremony_intervention`:

1. Preserve the participant's own request in `message`. Use `opinion`,
   `investigation`, or `action` for `kind`. Declare `role_kind` for the seat
   making the request, and likewise on every later call about that item:
   responding, closing it, and collecting evidence for it.
2. Omit `target_role_ids` when addressing the whole table. Supply explicit
   role ids when the participant named a specialist.
3. When the participant selects an option proposed in an earlier intervention,
   include `provenance` with the source intervention, the role whose response
   proposed it, and the selected role. Preserve the participant's specific
   wording; a selection is still not authorization for a consequential action.
4. For a read-only evidence request, call
   `made_collect_ceremony_evidence` when the embedded host configured the
   requested `source_id`. Preserve the participant's exact request in `query`
   and put safe structured selectors, such as the service and time window, in
   `details`. The returned non-empty evidence pack is recorded as that role's
   response.
5. Otherwise obtain the actual opinion, evidence, or action result with the
   host's available capabilities, then record each targeted role's contribution
   with `made_respond_to_ceremony_intervention`. Never turn an absent source
   or empty result into evidence; report the block and leave the intervention
   open.
6. Leave the intervention open until the requesting participant explicitly
   says they are satisfied or asks to close it. Only then call
   `made_close_ceremony_intervention` as that requesting role.

An `action` intervention is not human authorization and never bypasses a
ceremony guard or host permission. Resolve ambiguous operational requests to
the safe read-only meaning: inspect logs, query a database without writes, and
peek at queue metadata without consuming messages. Ask for explicit approval
before any consequential mutation.

## Reports

When the user asks for a ceremony report, confirm through discovery that
`made_generate_ceremony_report` is available, select exact ceremony ids, and
call it. Treat `structuredContent.report_markdown` as the generated artifact.
The tool is read-only and returns `persisted: false`; save it only through a
host-authorized file or document operation and report that destination
separately.
