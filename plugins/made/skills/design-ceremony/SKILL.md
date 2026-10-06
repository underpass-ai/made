---
name: design-ceremony
description: Design or revise a MADE ceremony, working session, review loop or human approval procedure from user intent before publication.
---

# Design a ceremony

Decision rules. The running server's `made_get_help` (`audience: "agent"`)
carries the full sequences; `tools/list` carries the exact schemas.

1. **Establish the decision or artifact first.** Then the roles that produce
   it, the stages in order, the outputs, and any decision that must stay with
   a person. Keep the user's vocabulary in objectives and instructions; ids
   are `lower_snake_case`.
2. **Call `made_design_ceremony` for a draft.** Its fields are top level
   (`name`, `objective`, `outputs`, `participants`, `stages` or `pattern`,
   `final_approval`). A refusal names the missing or undeclared field and the
   two accepted shapes; fix the call, do not guess around it.
3. **Treat the YAML as a draft even when `publishable` is true.** Explain
   ownership, sequence, outputs and the approval boundary to the user. Revise
   with another design call; compare versions with
   `made_diff_ceremony_definitions`. To revise something published, read it
   back with `made_get_ceremony_definition` rather than from memory.
4. **Use a pattern when the shape is one of the eight.** `sequential`,
   `concurrent`, `broadcast_collect`, `group_chat`, `maker_checker`,
   `handoff`, `magentic`, `advisor`. The designer expands it; `x-pattern` is
   metadata, never runtime semantics. Branching outcomes need explicit YAML
   validated with `made_validate_ceremony_draft`.
5. **A human guard is a seat only a person fills.** Add `final_approval` when
   a person's decision gates completion. Role names are vocabulary, not
   authority: an `INTEGRATOR` cannot approve for a person, bypass review or
   declare unverified work complete. Keep implementers, an independent
   reviewer and the human guard as separate roles.
6. **Bounded repetition is not retry.** A `repeat` needs `max_iterations`, an
   `output_field` and an exact `equals`, and ends with a declared exit. Give
   a slow stage its own `timeout_seconds` instead of raising the global one.
7. **Publish only the exact reviewed YAML, only when asked.** Publication
   binds name, version and content; changed content is a new version.
   Starting is a separate action, done with the `run-ceremony` skill.
8. **Several dependent procedures are a system, not one big ceremony.**
   Design each, publish, then compose them with `made_design_agentic_system`
   if that group is listed. If it is not, the tool profile hides it: say
   which group to add rather than improvising.

Host execution profiles (model, reasoning, fallback) are the host's and never
part of the definition. Declaring a role creates no agent; concurrent states
expose claimable work, they do not spawn workers.
