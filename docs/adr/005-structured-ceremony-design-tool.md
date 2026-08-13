# ADR-005: Ceremony design accepts intent and returns an unpublished draft

Status: Accepted

## Context

The existing authoring tools begin after a YAML document already exists.
That leaves every MCP host to reconstruct the ceremony schema, topology,
automated guards and role actions before the engine can help. Validation can
reject a bad draft, but it cannot distinguish deliberate structure from a
mechanical authoring mistake.

The engine must not become a product-specific planner or claim that a model's
proposal is authority. It also must not publish or execute merely because an
author asked for a design.

## Decision

The embedded Choreographer plugin exposes `choreo_design_ceremony`. It accepts
structured, domain-neutral intent: one objective, inputs, outputs,
participants, ordered stages and an optional final human approval.

The first version deliberately generates a linear topology. It owns the
mechanical declarations that follow from that intent:

- one state and step per ordered stage;
- one automated step-completion guard per transition;
- role actions for owned steps and transitions;
- an optional human guard on the terminal transition;
- timeout and retry declarations;
- a complete YAML `CeremonyDefinitionDraft`.

The generated YAML is parsed and analysed by the existing authoring path before
being returned. The response includes the whole YAML and its accumulated
analysis, and states that it is neither published nor started.

The host still owns semantic judgement: what the objective is, who should sit
at the session, what each stage must achieve and whether the draft reflects the
user's intent. Branching, alternative terminal decisions and arbitrary graphs
remain explicit YAML authoring until a future intent contract can express them
without ambiguity.

## Consequences

`design → validate/explain → human review → publish → start` is the supported
authoring path. Design is read-only and cannot record approval. A human guard
in the draft expresses a future requirement; it is not evidence that the
requirement was met.

The tool is an embedded-plugin extension, not a gRPC RPC. Remote services do
not advertise a capability they cannot honor, and the plugin can evolve the
authoring UX without changing the execution API.
