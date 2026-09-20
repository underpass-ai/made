//! MCP wire-protocol helpers and tool catalog.
//!
//! Hand-rolled JSON-RPC 2.0 + MCP `tools/*` shapes. The adapter owns every
//! byte that crosses stdio and projects one catalog over its active backend.

mod artifact_schemas;
mod authorization_actions;
mod authorization_schemas;
mod budget_schemas;
mod catalog;
#[cfg(any(feature = "embedded", feature = "grpc"))]
mod ceremony_journal_verdict_view;
mod ceremony_pattern_catalog;
mod ceremony_schemas;
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
mod request_gate;
mod result_envelopes;
mod schema_primitives;
mod struct_numbers;
#[cfg(test)]
mod tests;
mod tool_error;
mod tool_error_code;

/// MCP protocol version we advertise.
pub(crate) const PROTOCOL_VERSION: &str = "2024-11-05";
mod tool_names;

pub(crate) use catalog::{available_tool_catalog, tools_list_result};
pub(crate) use ceremony_pattern_catalog::{design_pattern_catalog, ROUNDTABLE_FIXED_ORDER_ID};
// Only a backend applies the rule; a build with neither would carry a
// function nothing calls.
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use ceremony_journal_verdict_view::CeremonyJournalVerdictView;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use ceremony_schemas::REPORT_IS_PERSISTED;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use default_idempotency_key::default_idempotency_key;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use default_lease_owner::default_lease_owner_id;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use default_lease_ttl::{
    CLAIM_CEREMONY_STEP_LEASE_TTL_MS, RUN_CEREMONY_LEASE_TTL_MS, RUN_CEREMONY_STEP_LEASE_TTL_MS,
};
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use design_dynamic_fields::validate_design_dynamic_fields;
pub(crate) use initialization::initialize_result;
pub(crate) use request_gate::validate_tool_request;
pub(crate) use result_envelopes::{
    jsonrpc_error, jsonrpc_result, tool_error_result, tool_success_result,
};
pub(crate) use struct_numbers::normalise_numbers;
pub use tool_error::ToolError;
pub use tool_error_code::ToolErrorCode;
// Only the tests ask which tools this server owns; the catalog and the
// gate reach the predicate through `tool_names` directly.
#[cfg(test)]
pub(crate) use tool_names::is_server_tool;
pub(crate) use tool_names::{
    is_grpc_tool, ABORT_ARTIFACT_UPLOAD_TOOL, ACCEPT_CHILD_COMPLETION_TOOL,
    ADOPT_EXECUTION_RECEIPT_TOOL, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    ASSERT_CEREMONY_REASON_TOOL, BEGIN_ARTIFACT_UPLOAD_TOOL, BIND_CEREMONY_PARTICIPANTS_TOOL,
    CANCEL_CEREMONY_TOOL, CLAIM_CEREMONY_STEP_TOOL, CLOSE_CEREMONY_INTERVENTION_TOOL,
    COLLECT_CEREMONY_EVIDENCE_TOOL, COMMIT_ARTIFACT_UPLOAD_TOOL, COMPLETE_CEREMONY_STEP_TOOL,
    COMPLETE_EXECUTION_RECEIPT_TOOL, DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL,
    DIFF_CEREMONY_DEFINITIONS_TOOL, DISCOVER_CAPABILITIES_TOOL, ENFORCE_CEREMONY_DEADLINES_TOOL,
    EXPLAIN_CEREMONY_DRAFT_TOOL, GENERATE_CEREMONY_REPORT_TOOL, GET_ARTIFACT_TOOL,
    GET_BUDGET_REPORT_TOOL, GET_CEREMONY_AGENT_TOOL, GET_CEREMONY_INSTANCE_TOOL,
    GET_CEREMONY_TRANSCRIPT_TOOL, GET_EXECUTION_RECEIPT_TOOL, GET_HELP_TOOL, GET_METRICS_TOOL,
    GET_STATUS_TOOL, INSPECT_EXECUTION_RECOVERY_TOOL, LIST_ARTIFACTS_TOOL,
    LIST_CEREMONY_AGENTS_TOOL, LIST_CEREMONY_INSTANCES_TOOL, LIST_PENDING_BUDGET_RESERVATIONS_TOOL,
    PAUSE_CEREMONY_TOOL, PREPARE_CEREMONY_CHILDREN_TOOL, PUBLISH_CEREMONY_DEFINITION_TOOL,
    PULL_CEREMONY_EVENTS_TOOL, PUT_ARTIFACT_CHUNK_TOOL, READ_ARTIFACT_CHUNK_TOOL,
    READ_CEREMONY_EVENTS_TOOL, RECOVER_CEREMONY_CHILDREN_TOOL, RENEW_CEREMONY_STEP_LEASE_TOOL,
    REPORT_CEREMONY_AGENT_STATUS_TOOL, REQUEST_CEREMONY_INTERVENTION_TOOL,
    RESPOND_TO_CEREMONY_INTERVENTION_TOOL, RESUME_CEREMONY_TOOL, RUN_CEREMONY_STEP_TOOL,
    RUN_CEREMONY_TOOL, SEARCH_CEREMONY_INSTANCES_TOOL, START_CEREMONY_TOOL,
    START_PUBLISHED_CEREMONY_TOOL, STREAM_CEREMONY_TOOL, TOMBSTONE_ARTIFACT_TOOL,
    VALIDATE_CEREMONY_DRAFT_TOOL, VERIFY_CEREMONY_JOURNAL_TOOL,
};

#[cfg(test)]
use catalog::grpc_tool_catalog;
#[cfg(test)]
use general_schemas::{output_contract_schema, task_schema};
#[cfg(test)]
use tool_names::GRPC_TOOL_NAMES;
pub(crate) use tool_names::{
    INSPECT_CEREMONY_RESUME_TOOL, PLAN_CEREMONY_SUCCESSOR_TOOL, RECORD_CEREMONY_HOST_HANDOFF_TOOL,
    START_CEREMONY_SUCCESSOR_TOOL,
};
