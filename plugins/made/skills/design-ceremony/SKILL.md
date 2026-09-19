---
name: design-ceremony
description: Design or revise a MADE ceremony, working session, review loop or human approval procedure from user intent before publication.
---

# Design a ceremony

Use `made_design_ceremony` for structured intent. Discover capabilities first
when the active version/backend is uncertain; use `made_get_help` with
`audience: "agent"` for the running server's guidance.

Establish the question or artifact to resolve, required context, participant
roles, stage ownership and outputs. Preserve the user's vocabulary in the
objective and instructions; use `lower_snake_case` ids. Give roles intervention
capabilities only when they need to request or respond to live agenda items.
Those capabilities do not authorize external actions.

Use peer `review_rounds` only with at least two agents. Use a bounded repeat
only for successful work that must recur until a structured stop condition:
provide `max_iterations` (1–1000), a top-level `output_field` and exact JSON
`equals`. A repeat is different from retrying failed work. Add
`final_approval` when a person's decision must gate completion.

When the work has an integration responsibility, use an explicit business
role such as `INTEGRATOR` and give it concrete integration steps and
transitions. Keep implementers, an independent reviewer, and any human guard
as separate declared responsibilities. Role names are vocabulary, not
authority: the definition's allowed actions and guards remain the source of
authorization, so an Integrator must not be presented as able to approve for a
person, bypass review, or declare unverified work complete. Use the executable
[integrator delivery example](../../../../docs/authoring/examples/integrator-delivery.yaml)
as the reference shape. Do not imply that this vocabulary creates live agent
rosters, activity reporting, or intervention delivery; those capabilities are
separate future work.

Treat returned YAML as a draft even when `publishable` is true. Inspect its
analysis and explain ownership, sequence, outputs and approval boundary to
the user. Revise with another design call; compare versions with
`made_diff_ceremony_definitions` when useful. The designer produces linear
stages. For branching outcomes, write explicit YAML and validate/explain it
through the available tools.

Publish the exact reviewed YAML only when the user has asked for that
publication. Publication binds immutable name/version/content; changed
content requires a new version. Starting is a separate action, performed when
the user asks to run it. Use the `run-ceremony` skill for execution, especially
when human guards require incremental progress.

Concurrent states expose claimable work; they do not spawn host agents.
Source-state joins and the global `all_steps_completed` guard have different
scope. A warning that the global guard can depend on downstream work needs
review, not a claim that the engine has repaired the draft automatically.
