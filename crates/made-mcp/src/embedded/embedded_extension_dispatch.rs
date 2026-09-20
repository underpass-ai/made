use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::protocol::{tool_success_result, ToolError};

// Every extension dispatch is a child of the dispatcher that routes it.
// The parent module is at its budget and grows with every capability,
// and a family belongs with the one thing that calls it anyway.
pub(super) mod embedded_agent_status_dispatch;
pub(super) mod embedded_agentic_system_dispatch;
pub(super) mod embedded_artifact_dispatch;
pub(super) mod embedded_authorization_dispatch;
pub(super) mod embedded_budget_dispatch;
pub(super) mod embedded_council_dispatch;
pub(super) mod embedded_council_journal_dispatch;
pub(super) mod embedded_succession_dispatch;
mod intervention_delivery;
mod intervention_delivery_presenter;

pub(super) fn handles(name: &str) -> bool {
    embedded_authorization_dispatch::handles(name)
        || embedded_council_journal_dispatch::handles(name)
        || embedded_budget_dispatch::handles(name)
        || embedded_council_dispatch::handles(name)
        || embedded_artifact_dispatch::handles(name)
        || embedded_agent_status_dispatch::handles(name)
        || intervention_delivery::handles(name)
        || embedded_agentic_system_dispatch::handles(name)
        || embedded_succession_dispatch::handles(name)
}

pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Option<Result<Value, ToolError>> {
    if embedded_authorization_dispatch::handles(name) {
        return Some(
            embedded_authorization_dispatch::dispatch(made, name, arguments)
                .await
                .map(tool_success_result),
        );
    }
    if embedded_council_journal_dispatch::handles(name) {
        return Some(
            embedded_council_journal_dispatch::dispatch(made, name, arguments)
                .await
                .map(tool_success_result),
        );
    }
    if embedded_budget_dispatch::handles(name) {
        return Some(
            embedded_budget_dispatch::dispatch(made, name, arguments)
                .await
                .map(tool_success_result),
        );
    }
    if embedded_council_dispatch::handles(name) {
        return Some(embedded_council_dispatch::dispatch(made, name, arguments).await);
    }
    if embedded_artifact_dispatch::handles(name) {
        return Some(embedded_artifact_dispatch::dispatch(made, name, arguments).await);
    }
    if embedded_agent_status_dispatch::handles(name) {
        return Some(embedded_agent_status_dispatch::dispatch(made, name, arguments).await);
    }
    if intervention_delivery::handles(name) {
        return Some(intervention_delivery::dispatch(made, name, arguments).await);
    }
    if embedded_agentic_system_dispatch::handles(name) {
        return Some(
            embedded_agentic_system_dispatch::dispatch(made, name, arguments)
                .await
                .map(tool_success_result),
        );
    }
    if embedded_succession_dispatch::handles(name) {
        return Some(embedded_succession_dispatch::dispatch(made, name, arguments).await);
    }
    None
}
