//! Tool-name → gRPC RPC dispatch.
//!
//! One entry per MADE RPC. The dispatcher maps JSON arguments through
//! request mappers, calls the generated tonic client, and maps the response.

use made_mcp_proto::v1 as pb;
use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use serde_json::{json, Value};
use tonic::transport::Channel;

use crate::protocol::ToolError;

use super::json_to_proto as j2p;
use super::proto_to_json as p2j;
use super::streaming;

mod agentic_system_dispatch;
mod agentic_system_presenter;
mod agentic_system_requests;
mod artifact_dispatch;
mod artifact_requests;
mod authorization_dispatch;
mod authorization_presenter;
mod authorization_requests;
mod budget_dispatch;
mod ceremony_agent_status_requests;
mod ceremony_history_requests;
mod ceremony_move_dispatch;
mod ceremony_read_dispatch;
mod ceremony_requests;
mod children_dispatch;
mod council_journal_dispatch;
mod design_ceremony_request;
mod execution_receipt_dispatch;
mod general_dispatch;
mod general_requests;
mod host_handoff_dispatch;
mod host_handoff_presenter;
mod host_handoff_requests;
mod integrator_loop_dispatch;
mod integrator_loop_presenter;
mod integrator_loop_requests;
mod intervention_delivery_dispatch;
mod intervention_delivery_presenter;
mod intervention_delivery_requests;
mod intervention_request_builders;
mod lifecycle_dispatch;
mod lifecycle_requests;
mod request_error;
mod request_metadata_client;
mod succession_dispatch;
mod succession_presenter;
mod succession_requests;

use request_error::bad_request;

// One rule for the runner an omitted `lease_owner_id` becomes; the
// one-shot run mapper lives in `json_to_proto` and uses the same one.
pub(in crate::grpc) use ceremony_requests::{lease_owner_id, lease_ttl_ms};
#[cfg(test)]
mod schema_gate;

/// Dispatch one tool call. Returns the **structured content** of the
/// MCP tool result (just the JSON; the caller wraps it in
/// `tool_success_result`).
#[allow(clippy::too_many_lines, clippy::result_large_err)] // one arm per tool; tonic's interceptor contract returns Status
pub(crate) async fn dispatch(
    channel: Channel,
    name: &str,
    arguments: &Value,
    traceparent: &str,
    request_id: &str,
    target_digest: &str,
    approval_decision_id: Option<&str>,
) -> Result<Value, ToolError> {
    let mut client = request_metadata_client::build(
        channel,
        traceparent,
        request_id,
        target_digest,
        approval_decision_id,
    )?;
    if authorization_dispatch::handles(name) {
        return authorization_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if council_journal_dispatch::handles(name) {
        return council_journal_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if general_dispatch::handles(name) {
        return general_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if children_dispatch::handles(name) {
        return children_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if lifecycle_dispatch::handles(name) {
        return lifecycle_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if ceremony_read_dispatch::handles(name) {
        return ceremony_read_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if artifact_dispatch::handles(name) {
        return artifact_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if execution_receipt_dispatch::handles(name) {
        return execution_receipt_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if host_handoff_dispatch::handles(name) {
        return host_handoff_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if intervention_delivery_dispatch::handles(name) {
        return intervention_delivery_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if integrator_loop_dispatch::handles(name) {
        return integrator_loop_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if agentic_system_dispatch::handles(name) {
        return agentic_system_dispatch::dispatch(&mut client, name, arguments).await;
    }
    if succession_dispatch::handles(name) {
        return succession_dispatch::dispatch(&mut client, name, arguments).await;
    }

    ceremony_move_dispatch::dispatch(&mut client, name, arguments).await
}
