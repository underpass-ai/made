//! Running a protocol, and everything a seat does inside one.
//!
//! The largest family in the catalogue, and the one every other entry
//! is ordered around: a caller reading it top to bottom reads a
//! session's life, from opening one to closing the last thing somebody
//! asked in it.

use serde_json::{json, Value};

use super::{host_handoff_catalog, succession_catalog};

use crate::mcp_protocol::protocol_schemas::{
    accept_child_completion_schema, close_protocol_intervention_schema,
    enforce_protocol_deadlines_schema, prepare_protocol_children_schema,
    protocol_guard_approval_schema, protocol_guard_deferral_schema, protocol_instance_schema,
    protocol_lifecycle_control_schema, protocol_search_schema, protocol_transition_schema,
    recover_protocol_children_schema, request_protocol_intervention_schema,
    respond_to_protocol_intervention_schema, run_protocol_schema, run_protocol_step_schema,
    start_protocol_schema, start_published_protocol_schema,
};
use crate::mcp_protocol::schema_primitives::tool_def;
use crate::mcp_protocol::tool_names::{
    ACCEPT_CHILD_COMPLETION_TOOL, APPLY_PROTOCOL_TRANSITION_TOOL, APPROVE_PROTOCOL_GUARD_TOOL,
    CANCEL_PROTOCOL_TOOL, CLOSE_PROTOCOL_INTERVENTION_TOOL, DEFER_PROTOCOL_GUARD_TOOL,
    ENFORCE_PROTOCOL_DEADLINES_TOOL, GET_PROTOCOL_INSTANCE_TOOL, LIST_PROTOCOL_INSTANCES_TOOL,
    PAUSE_PROTOCOL_TOOL, PREPARE_PROTOCOL_CHILDREN_TOOL, RECOVER_PROTOCOL_CHILDREN_TOOL,
    REQUEST_PROTOCOL_INTERVENTION_TOOL, RESPOND_TO_PROTOCOL_INTERVENTION_TOOL,
    RESUME_PROTOCOL_TOOL, RUN_PROTOCOL_STEP_TOOL, RUN_PROTOCOL_TOOL,
    SEARCH_PROTOCOL_INSTANCES_TOOL, START_PROTOCOL_TOOL, START_PUBLISHED_PROTOCOL_TOOL,
};

#[allow(clippy::too_many_lines)] // one ordered table of a session's life
pub(super) fn protocol_lifecycle_tool_catalog() -> Vec<Value> {
    vec![
tool_def(
            RUN_PROTOCOL_TOOL,
            "Execute a declarative protocol YAML definition and return final state, step trace, and Mermaid sequence diagram.",
            run_protocol_schema(),
        ),
        tool_def(
            GET_PROTOCOL_INSTANCE_TOOL,
            "Inspect a persistent protocol instance, including step status and blocking guards.",
            protocol_instance_schema(),
        ),
        tool_def(
            LIST_PROTOCOL_INSTANCES_TOOL,
            "Discover protocol instances available to this backend so a host can resume one after losing its local conversation context.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        tool_def(SEARCH_PROTOCOL_INSTANCES_TOOL, "Search one bounded protocol page.", protocol_search_schema()),
        tool_def(
            START_PROTOCOL_TOOL,
            "Mount a protocol YAML definition and start a persistent in-process instance without advancing it.",
            start_protocol_schema(),
        ),
        tool_def(
            START_PUBLISHED_PROTOCOL_TOOL,
            "Start a protocol from a published version, binding the instance to that definition's digest so which one ran can be checked afterwards rather than taken on trust.",
            start_published_protocol_schema(),
        ),
        tool_def(
            RUN_PROTOCOL_STEP_TOOL,
            "Execute one declared step on a started protocol instance and persist its result.",
            run_protocol_step_schema(),
        ),
        tool_def(
            PREPARE_PROTOCOL_CHILDREN_TOOL,
            "Claim a spawning step, seal its deterministic child plan, open every child exactly once, and complete the parent step only after all openings are verified.",
            prepare_protocol_children_schema(),
        ),
        tool_def(
            ACCEPT_CHILD_COMPLETION_TOOL,
            "Verify a named ProtocolCompleted record in a child's intact journal, plus its lineage, binding and opening snapshot, before recording it in the parent group.",
            accept_child_completion_schema(),
        ),
        tool_def(
            RECOVER_PROTOCOL_CHILDREN_TOOL,
            "Advance durable child-plan and child-completion recovery from its owned event cursor. Notifications may wake this operation but never substitute for cursor evidence.",
            recover_protocol_children_schema(),
        ),
        tool_def(
            APPLY_PROTOCOL_TRANSITION_TOOL,
            "Apply one enabled protocol transition and return the updated persistent instance.",
            protocol_transition_schema(),
        ),
        tool_def(
            PAUSE_PROTOCOL_TOOL,
            "Pause admission of new protocol work while already accepted claims and child openings drain.",
            protocol_lifecycle_control_schema(true),
        ),
        tool_def(
            RESUME_PROTOCOL_TOOL,
            "Resume admission of new work without shifting absolute deadlines or live claim fences.",
            protocol_lifecycle_control_schema(false),
        ),
        host_handoff_catalog::record_tool(),
        host_handoff_catalog::inspect_tool(),
        succession_catalog::plan_tool(),
        succession_catalog::start_tool(),
        tool_def(
            CANCEL_PROTOCOL_TOOL,
            "Irreversibly end a protocol without cascading to children or rolling back external work.",
            protocol_lifecycle_control_schema(true),
        ),
        tool_def(
            ENFORCE_PROTOCOL_DEADLINES_TOOL,
            "Evaluate sealed protocol, state, and step deadlines using the engine clock and persist deterministic timeout decisions.",
            enforce_protocol_deadlines_schema(),
        ),
        tool_def(
            APPROVE_PROTOCOL_GUARD_TOOL,
            "Record an explicit human approval for a currently-blocking human guard. Call only after the human has authorized it.",
            protocol_guard_approval_schema(),
        ),
        tool_def(
            DEFER_PROTOCOL_GUARD_TOOL,
            "Record an explicit human deferral without satisfying the guard or inferring authorization.",
            protocol_guard_deferral_schema(),
        ),
        tool_def(
            REQUEST_PROTOCOL_INTERVENTION_TOOL,
            "Open a participant-requested opinion, investigation, or action on the live protocol table. This coordinates the request; it does not authorize external mutations.",
            request_protocol_intervention_schema(),
        ),
        tool_def(
            RESPOND_TO_PROTOCOL_INTERVENTION_TOOL,
            "Record one targeted role's response to an open protocol intervention.",
            respond_to_protocol_intervention_schema(),
        ),
        tool_def(
            CLOSE_PROTOCOL_INTERVENTION_TOOL,
            "Close an open protocol intervention as its requesting role.",
            close_protocol_intervention_schema(),
        ),
    ]
}
