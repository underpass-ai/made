//! MCP wire-protocol helpers and tool catalog.
//!
//! Hand-rolled JSON-RPC 2.0 + MCP `tools/*` shapes. The adapter owns every
//! byte that crosses stdio and projects one catalog over its active backend.

mod artifact_schemas;
mod authorization_actions;
mod authorization_schemas;
mod budget_schemas;
mod catalog;
mod default_idempotency_key;
mod default_lease_owner;
mod default_lease_ttl;
#[cfg(any(feature = "embedded", feature = "grpc"))]
mod design_dynamic_fields;
#[cfg(test)]
mod editions_matrix_tests;
mod general_schemas;
mod initialization;
#[cfg(test)]
mod parity_tests;
#[cfg(any(feature = "embedded", feature = "grpc"))]
mod protocol_journal_verdict_view;
mod protocol_pattern_catalog;
mod protocol_schemas;
mod request_gate;
mod result_envelopes;
mod schema_primitives;
mod struct_numbers;
#[cfg(test)]
mod tests;
mod tool_actions;
mod tool_error;
mod tool_error_code;

/// MCP protocol version we advertise.
pub(crate) const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
mod tool_names;

pub(crate) use catalog::{available_tool_catalog, tools_list_result};
pub(crate) use protocol_pattern_catalog::{design_pattern_catalog, ROUNDTABLE_FIXED_ORDER_ID};
// Only a backend applies the rule; a build with neither would carry a
// function nothing calls.
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use default_idempotency_key::default_idempotency_key;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use default_lease_owner::default_lease_owner_id;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use default_lease_ttl::{
    CLAIM_PROTOCOL_STEP_LEASE_TTL_MS, RUN_PROTOCOL_LEASE_TTL_MS, RUN_PROTOCOL_STEP_LEASE_TTL_MS,
};
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use design_dynamic_fields::validate_design_dynamic_fields;
pub(crate) use initialization::initialize_result;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use protocol_journal_verdict_view::ProtocolJournalVerdictView;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use protocol_schemas::REPORT_IS_PERSISTED;
pub(crate) use protocol_schemas::{
    agentic_system_advance_schema, agentic_system_design_schema, agentic_system_diagram_schema,
    agentic_system_execution_schema, agentic_system_instantiate_schema, agentic_system_list_schema,
    agentic_system_publish_schema, agentic_system_read_schema,
};
pub(crate) use request_gate::validate_tool_request;
pub(crate) use result_envelopes::{
    jsonrpc_error, jsonrpc_result, tool_error_result, tool_success_result,
};
pub(crate) use struct_numbers::normalise_numbers;
pub(crate) use tool_actions::action_name_for_tool;
pub use tool_error::ToolError;
pub use tool_error_code::ToolErrorCode;
// The catalog and the gate reach the predicate through `tool_names`
// directly; the tool profile asks it so that a host can always ask what
// it is talking to, whatever else the profile hides.
pub(crate) use tool_names::is_server_tool;
// Names only, for the readers that join tools to policy actions and
// would otherwise build the whole catalog to read its names.
pub(crate) use tool_names::GRPC_TOOL_NAMES;
pub(crate) use tool_names::{
    is_grpc_tool, ABORT_ARTIFACT_UPLOAD_TOOL, ACCEPT_CHILD_COMPLETION_TOOL,
    ACKNOWLEDGE_PROTOCOL_AGENT_INTERVENTION_TOOL, ADOPT_EXECUTION_RECEIPT_TOOL,
    APPLY_PROTOCOL_TRANSITION_TOOL, APPROVE_PROTOCOL_GUARD_TOOL, ASSERT_PROTOCOL_REASON_TOOL,
    BEGIN_ARTIFACT_UPLOAD_TOOL, BIND_PROTOCOL_PARTICIPANTS_TOOL, CANCEL_PROTOCOL_TOOL,
    CLAIM_PROTOCOL_STEP_TOOL, CLOSE_PROTOCOL_INTERVENTION_TOOL, COLLECT_PROTOCOL_EVIDENCE_TOOL,
    COMMIT_ARTIFACT_UPLOAD_TOOL, COMPLETE_EXECUTION_RECEIPT_TOOL, COMPLETE_PROTOCOL_STEP_TOOL,
    DEFER_PROTOCOL_GUARD_TOOL, DESIGN_PROTOCOL_TOOL, DIFF_PROTOCOL_DEFINITIONS_TOOL,
    DISCOVER_CAPABILITIES_TOOL, ENFORCE_PROTOCOL_DEADLINES_TOOL, EXPLAIN_PROTOCOL_DRAFT_TOOL,
    GENERATE_PROTOCOL_REPORT_TOOL, GET_ARTIFACT_TOOL, GET_BUDGET_REPORT_TOOL,
    GET_EXECUTION_RECEIPT_TOOL, GET_HELP_TOOL, GET_METRICS_TOOL, GET_PROTOCOL_AGENT_TOOL,
    GET_PROTOCOL_DEFINITION_TOOL, GET_PROTOCOL_INSTANCE_TOOL, GET_PROTOCOL_INTERVENTION_TOOL,
    GET_PROTOCOL_TRANSCRIPT_TOOL, GET_STATUS_TOOL, INSPECT_EXECUTION_RECOVERY_TOOL,
    LIST_ARTIFACTS_TOOL, LIST_PENDING_BUDGET_RESERVATIONS_TOOL, LIST_PROTOCOL_AGENTS_TOOL,
    LIST_PROTOCOL_DEFINITIONS_TOOL, LIST_PROTOCOL_INSTANCES_TOOL, LIST_PROTOCOL_INTERVENTIONS_TOOL,
    PAUSE_PROTOCOL_TOOL, PREPARE_PROTOCOL_CHILDREN_TOOL, PUBLISH_PROTOCOL_DEFINITION_TOOL,
    PULL_PROTOCOL_AGENT_INTERVENTIONS_TOOL, PULL_PROTOCOL_EVENTS_TOOL, PUT_ARTIFACT_CHUNK_TOOL,
    READ_ARTIFACT_CHUNK_TOOL, READ_PROTOCOL_EVENTS_TOOL, RECOVER_PROTOCOL_CHILDREN_TOOL,
    RENEW_PROTOCOL_STEP_LEASE_TOOL, REPORT_PROTOCOL_AGENT_STATUS_TOOL,
    REQUEST_PROTOCOL_INTERVENTION_TOOL, RESPOND_TO_PROTOCOL_INTERVENTION_TOOL,
    RESUME_PROTOCOL_TOOL, RUN_PROTOCOL_STEP_TOOL, RUN_PROTOCOL_TOOL,
    SEARCH_PROTOCOL_INSTANCES_TOOL, START_PROTOCOL_TOOL, START_PUBLISHED_PROTOCOL_TOOL,
    STREAM_PROTOCOL_TOOL, TOMBSTONE_ARTIFACT_TOOL, VALIDATE_PROTOCOL_DRAFT_TOOL,
    VERIFY_PROTOCOL_JOURNAL_TOOL,
};

#[cfg(test)]
use catalog::grpc_tool_catalog;
#[cfg(test)]
use general_schemas::{output_contract_schema, task_schema};
pub(crate) use tool_names::{
    ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL, AWAIT_INTEGRATOR_ATTENTION_TOOL,
    BIND_PROTOCOL_INTEGRATOR_TOOL, GET_PROTOCOL_INTEGRATOR_BINDING_TOOL,
    LIST_ATTENTION_DELIVERIES_TOOL,
};
pub(crate) use tool_names::{
    ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL, DESIGN_AGENTIC_SYSTEM_TOOL,
    GET_AGENTIC_SYSTEM_EXECUTION_TOOL, GET_AGENTIC_SYSTEM_TOOL, INSTANTIATE_AGENTIC_SYSTEM_TOOL,
    LIST_AGENTIC_SYSTEMS_TOOL, PUBLISH_AGENTIC_SYSTEM_TOOL, RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL,
    VALIDATE_AGENTIC_SYSTEM_TOOL,
};
pub(crate) use tool_names::{
    INSPECT_PROTOCOL_RESUME_TOOL, PLAN_PROTOCOL_SUCCESSOR_TOOL, RECORD_PROTOCOL_HOST_HANDOFF_TOOL,
    START_PROTOCOL_SUCCESSOR_TOOL,
};
