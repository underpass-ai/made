---
name: design-ceremony
description: Design a new Choreographer ceremony or working session from user intent, or revise a proposed ceremony before publication. Use when the user asks to create, structure, plan, draft, or design a ceremony, mesa de trabajo, workflow, review loop, governed multi-agent session, or human approval flow.
---

# Design a Choreographer ceremony

Use `choreo_design_ceremony` before writing ceremony YAML yourself. Translate
the user's intent into the tool's structured fields while preserving their
vocabulary in the objective and stage instructions.

1. Establish the single question or artifact the ceremony must resolve.
2. Name the required context, output objects, participant roles and ordered
   stages. Use `lower_snake_case` for ceremony and stage ids.
3. Add `request_intervention` or `respond_to_intervention` only when that role
   needs to change the live agenda. These capabilities never authorize an
   external mutation.
4. Use `review_rounds` only with `num_agents >= 2`. Keep one agent for a stage
   that needs no peer exchange.
5. Add `final_approval` when completion depends on a person's explicit
   decision. This creates a human guard; it does not approve it.
6. Call `choreo_design_ceremony`. Treat its `definition_yaml` as a draft even
   when `publishable` is true. The tool neither publishes nor starts anything.
7. Read the returned design and analysis back to the user. Check that stage
   ownership, sequence, instructions, outputs and approval boundary match their
   intent. Use `choreo_explain_ceremony_draft` when a prose explanation helps.
8. Revise by calling `choreo_design_ceremony` again with changed intent. Use
   `choreo_diff_ceremony_definitions` when comparing an existing draft or
   published definition.
9. Call `choreo_publish_ceremony_definition` only after the user explicitly
   asks to publish the exact reviewed YAML. Publication is immutable by name
   and version; changed content requires a new version.
10. Start the published ceremony only when the user asks to run it. Follow the
    incremental flow in `run-ceremony` whenever a human guard exists.

The design tool currently produces linear ceremonies. If the request requires
branching decisions or multiple terminal outcomes, explain that limit and
author explicit YAML for those branches, then validate and explain it through
the MCP before asking to publish.
