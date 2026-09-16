//! MCP wire-protocol helpers and tool catalog.
//!
//! Hand-rolled JSON-RPC 2.0 + MCP `tools/*` shapes. The adapter owns every
//! byte that crosses stdio and projects one catalog over its active backend.

mod catalog;
mod ceremony_schemas;
mod default_lease_owner;
mod default_lease_ttl;
#[cfg(test)]
mod editions_matrix_tests;
mod general_schemas;
mod initialization;
#[cfg(test)]
mod parity_tests;
mod request_gate;
mod result_envelopes;
mod schema_primitives;
#[cfg(test)]
mod tests;
mod tool_error;
mod tool_error_code;

/// MCP protocol version we advertise.
pub(crate) const PROTOCOL_VERSION: &str = "2024-11-05";
mod tool_names;

pub(crate) use catalog::{available_tool_catalog, tools_list_result};
// Only a backend applies the rule; a build with neither would carry a
// function nothing calls.
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use ceremony_schemas::REPORT_IS_PERSISTED;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use default_lease_owner::default_lease_owner_id;
#[cfg(any(feature = "embedded", feature = "grpc"))]
pub(crate) use default_lease_ttl::{
    CLAIM_CEREMONY_STEP_LEASE_TTL_MS, RUN_CEREMONY_LEASE_TTL_MS, RUN_CEREMONY_STEP_LEASE_TTL_MS,
};
pub(crate) use initialization::initialize_result;
pub(crate) use request_gate::validate_tool_request;
pub(crate) use result_envelopes::{
    jsonrpc_error, jsonrpc_result, tool_error_result, tool_success_result,
};
pub use tool_error::ToolError;
pub use tool_error_code::ToolErrorCode;
// Only the tests ask which tools this server owns; the catalog and the
// gate reach the predicate through `tool_names` directly.
#[cfg(test)]
pub(crate) use tool_names::is_server_tool;
pub(crate) use tool_names::{
    is_grpc_tool, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    ASSERT_CEREMONY_REASON_TOOL, BIND_CEREMONY_PARTICIPANTS_TOOL, CLAIM_CEREMONY_STEP_TOOL,
    CLOSE_CEREMONY_INTERVENTION_TOOL, COLLECT_CEREMONY_EVIDENCE_TOOL, COMPLETE_CEREMONY_STEP_TOOL,
    DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL, DIFF_CEREMONY_DEFINITIONS_TOOL,
    DISCOVER_CAPABILITIES_TOOL, EXPLAIN_CEREMONY_DRAFT_TOOL, GENERATE_CEREMONY_REPORT_TOOL,
    GET_CEREMONY_INSTANCE_TOOL, GET_CEREMONY_TRANSCRIPT_TOOL, GET_HELP_TOOL, GET_METRICS_TOOL,
    GET_STATUS_TOOL, LIST_CEREMONY_INSTANCES_TOOL, PUBLISH_CEREMONY_DEFINITION_TOOL,
    READ_CEREMONY_EVENTS_TOOL, REQUEST_CEREMONY_INTERVENTION_TOOL,
    RESPOND_TO_CEREMONY_INTERVENTION_TOOL, RUN_CEREMONY_STEP_TOOL, RUN_CEREMONY_TOOL,
    START_CEREMONY_TOOL, START_PUBLISHED_CEREMONY_TOOL, VALIDATE_CEREMONY_DRAFT_TOOL,
};

#[cfg(test)]
use catalog::grpc_tool_catalog;
#[cfg(test)]
use general_schemas::{output_contract_schema, task_schema};
#[cfg(test)]
use tool_names::GRPC_TOOL_NAMES;
