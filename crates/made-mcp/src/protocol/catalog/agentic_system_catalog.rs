//! The nine agentic-system tools, as a client discovers them.
//!
//! Descriptions say what each one does *and* what it refuses, because
//! a caller that learns the refusals only by hitting them will have
//! designed around the wrong model first.

use crate::protocol::schema_primitives::tool_def;
use crate::protocol::{
    agentic_system_advance_schema, agentic_system_design_schema, agentic_system_diagram_schema,
    agentic_system_execution_schema, agentic_system_instantiate_schema, agentic_system_list_schema,
    agentic_system_publish_schema, agentic_system_read_schema,
    ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL, DESIGN_AGENTIC_SYSTEM_TOOL,
    GET_AGENTIC_SYSTEM_EXECUTION_TOOL, GET_AGENTIC_SYSTEM_TOOL, INSTANTIATE_AGENTIC_SYSTEM_TOOL,
    LIST_AGENTIC_SYSTEMS_TOOL, PUBLISH_AGENTIC_SYSTEM_TOOL, RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL,
    VALIDATE_AGENTIC_SYSTEM_TOOL,
};
use serde_json::Value;

pub(super) fn agentic_system_tool_catalog() -> Vec<Value> {
    vec![
        tool_def(
            DESIGN_AGENTIC_SYSTEM_TOOL,
            "Write down the level above a ceremony: business roles, logical participants, a collaboration topology, several published ceremonies composed together, and the supervision they run under. Saves against the revision you read, so a concurrent edit is refused rather than overwritten. Composes ceremonies by pin; it never copies or owns a definition. Unknown fields are refused.",
            agentic_system_design_schema(),
        ),
        tool_def(
            GET_AGENTIC_SYSTEM_TOOL,
            "Read one design at a named revision or at its head, with its content digest and the document in both YAML and JSON. Reading an earlier revision is how a run that pinned it stays explainable after the design has moved on.",
            agentic_system_read_schema("Design to read."),
        ),
        tool_def(
            LIST_AGENTIC_SYSTEMS_TOOL,
            "List the head of each design, optionally filtered by lifecycle. Bounded and resumable by the last identifier returned.",
            agentic_system_list_schema(),
        ),
        tool_def(
            VALIDATE_AGENTIC_SYSTEM_TOOL,
            "Resolve every pin against what is actually published and report every defect at once, each located at the element it is about: unpublished or changed pins, unfilled seats, unknown identifiers, an integrator that is not one, a violated independence rule, a required capability nobody supplies, input wiring to an output that is not declared, and a dependency cycle no bounded loop closes. An error blocks publication.",
            agentic_system_read_schema("Design to analyse."),
        ),
        tool_def(
            PUBLISH_AGENTIC_SYSTEM_TOOL,
            "Validate, then seal the revision you read so a run can pin it. Offering byte-identical content under a revision that already holds it answers already_published; offering different content under it is refused. A sealed revision never changes afterwards.",
            agentic_system_publish_schema(),
        ),
        tool_def(
            INSTANTIATE_AGENTIC_SYSTEM_TOOL,
            "Open a run of a sealed revision and start the ceremonies nothing blocks. The execution id is your idempotency key: asking twice answers with the run that exists. Offers say what your host can supply per logical participant; a participant you do not offer is recorded unavailable and the ceremonies needing it are skipped with the reason. Nothing is stood in for.",
            agentic_system_instantiate_schema(),
        ),
        tool_def(
            ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL,
            "Start whatever the run is now ready for, and send bounded loops round for another round. What can start is read back from the instances rather than from a cursor, so advancing twice with nothing changed starts nothing twice.",
            agentic_system_advance_schema(),
        ),
        tool_def(
            GET_AGENTIC_SYSTEM_EXECUTION_TOOL,
            "Read a run with what the design intended beside what its instances actually report, kept apart so a skipped ceremony never reads like one the design never had. The design returned is the revision the run pinned, not the head.",
            agentic_system_execution_schema(),
        ),
        tool_def(
            RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL,
            "Render the topology as Mermaid text and as an accessible list of the same content. With an execution id, the picture is of that run's pinned design with its observed progress. Turning the text into an image is the caller's job; no renderer is shipped here.",
            agentic_system_diagram_schema(),
        ),
    ]
}
