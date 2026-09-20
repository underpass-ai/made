//! Running a ceremony, and everything a seat does inside one.
//!
//! The largest family in the catalogue, and the one every other entry
//! is ordered around: a caller reading it top to bottom reads a
//! session's life, from opening one to closing the last thing somebody
//! asked in it.

use serde_json::{json, Value};

use super::{host_handoff_catalog, succession_catalog};

use crate::protocol::ceremony_schemas::{
    accept_child_completion_schema, ceremony_guard_approval_schema, ceremony_guard_deferral_schema,
    ceremony_instance_schema, ceremony_lifecycle_control_schema, ceremony_search_schema,
    ceremony_transition_schema, close_ceremony_intervention_schema,
    enforce_ceremony_deadlines_schema, prepare_ceremony_children_schema,
    recover_ceremony_children_schema, request_ceremony_intervention_schema,
    respond_to_ceremony_intervention_schema, run_ceremony_schema, run_ceremony_step_schema,
    start_ceremony_schema, start_published_ceremony_schema,
};
use crate::protocol::schema_primitives::tool_def;
use crate::protocol::tool_names::{
    ACCEPT_CHILD_COMPLETION_TOOL, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    CANCEL_CEREMONY_TOOL, CLOSE_CEREMONY_INTERVENTION_TOOL, DEFER_CEREMONY_GUARD_TOOL,
    ENFORCE_CEREMONY_DEADLINES_TOOL, GET_CEREMONY_INSTANCE_TOOL, LIST_CEREMONY_INSTANCES_TOOL,
    PAUSE_CEREMONY_TOOL, PREPARE_CEREMONY_CHILDREN_TOOL, RECOVER_CEREMONY_CHILDREN_TOOL,
    REQUEST_CEREMONY_INTERVENTION_TOOL, RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
    RESUME_CEREMONY_TOOL, RUN_CEREMONY_STEP_TOOL, RUN_CEREMONY_TOOL,
    SEARCH_CEREMONY_INSTANCES_TOOL, START_CEREMONY_TOOL, START_PUBLISHED_CEREMONY_TOOL,
};

#[allow(clippy::too_many_lines)] // one ordered table of a session's life
pub(super) fn ceremony_lifecycle_tool_catalog() -> Vec<Value> {
    vec![
tool_def(
            RUN_CEREMONY_TOOL,
            "Execute a declarative ceremony YAML definition and return final state, step trace, and Mermaid sequence diagram.",
            run_ceremony_schema(),
        ),
        tool_def(
            GET_CEREMONY_INSTANCE_TOOL,
            "Inspect a persistent ceremony instance, including step status and blocking guards.",
            ceremony_instance_schema(),
        ),
        tool_def(
            LIST_CEREMONY_INSTANCES_TOOL,
            "Discover ceremony instances available to this backend so a host can resume one after losing its local conversation context.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        tool_def(SEARCH_CEREMONY_INSTANCES_TOOL, "Search one bounded ceremony page.", ceremony_search_schema()),
        tool_def(
            START_CEREMONY_TOOL,
            "Mount a ceremony YAML definition and start a persistent in-process instance without advancing it.",
            start_ceremony_schema(),
        ),
        tool_def(
            START_PUBLISHED_CEREMONY_TOOL,
            "Start a ceremony from a published version, binding the instance to that definition's digest so which one ran can be checked afterwards rather than taken on trust.",
            start_published_ceremony_schema(),
        ),
        tool_def(
            RUN_CEREMONY_STEP_TOOL,
            "Execute one declared step on a started ceremony instance and persist its result.",
            run_ceremony_step_schema(),
        ),
        tool_def(
            PREPARE_CEREMONY_CHILDREN_TOOL,
            "Claim a spawning step, seal its deterministic child plan, open every child exactly once, and complete the parent step only after all openings are verified.",
            prepare_ceremony_children_schema(),
        ),
        tool_def(
            ACCEPT_CHILD_COMPLETION_TOOL,
            "Verify a named CeremonyCompleted record in a child's intact journal, plus its lineage, binding and opening snapshot, before recording it in the parent group.",
            accept_child_completion_schema(),
        ),
        tool_def(
            RECOVER_CEREMONY_CHILDREN_TOOL,
            "Advance durable child-plan and child-completion recovery from its owned event cursor. Notifications may wake this operation but never substitute for cursor evidence.",
            recover_ceremony_children_schema(),
        ),
        tool_def(
            APPLY_CEREMONY_TRANSITION_TOOL,
            "Apply one enabled ceremony transition and return the updated persistent instance.",
            ceremony_transition_schema(),
        ),
        tool_def(
            PAUSE_CEREMONY_TOOL,
            "Pause admission of new ceremony work while already accepted claims and child openings drain.",
            ceremony_lifecycle_control_schema(true),
        ),
        tool_def(
            RESUME_CEREMONY_TOOL,
            "Resume admission of new work without shifting absolute deadlines or live claim fences.",
            ceremony_lifecycle_control_schema(false),
        ),
        host_handoff_catalog::record_tool(),
        host_handoff_catalog::inspect_tool(),
        succession_catalog::plan_tool(),
        succession_catalog::start_tool(),
        tool_def(
            CANCEL_CEREMONY_TOOL,
            "Irreversibly end a ceremony without cascading to children or rolling back external work.",
            ceremony_lifecycle_control_schema(true),
        ),
        tool_def(
            ENFORCE_CEREMONY_DEADLINES_TOOL,
            "Evaluate sealed ceremony, state, and step deadlines using the engine clock and persist deterministic timeout decisions.",
            enforce_ceremony_deadlines_schema(),
        ),
        tool_def(
            APPROVE_CEREMONY_GUARD_TOOL,
            "Record an explicit human approval for a currently-blocking human guard. Call only after the human has authorized it.",
            ceremony_guard_approval_schema(),
        ),
        tool_def(
            DEFER_CEREMONY_GUARD_TOOL,
            "Record an explicit human deferral without satisfying the guard or inferring authorization.",
            ceremony_guard_deferral_schema(),
        ),
        tool_def(
            REQUEST_CEREMONY_INTERVENTION_TOOL,
            "Open a participant-requested opinion, investigation, or action on the live ceremony table. This coordinates the request; it does not authorize external mutations.",
            request_ceremony_intervention_schema(),
        ),
        tool_def(
            RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
            "Record one targeted role's response to an open ceremony intervention.",
            respond_to_ceremony_intervention_schema(),
        ),
        tool_def(
            CLOSE_CEREMONY_INTERVENTION_TOOL,
            "Close an open ceremony intervention as its requesting role.",
            close_ceremony_intervention_schema(),
        ),
    ]
}
