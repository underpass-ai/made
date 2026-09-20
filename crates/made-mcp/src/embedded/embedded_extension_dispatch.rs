use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::protocol::{tool_success_result, ToolError};

use super::{
    embedded_agent_status_dispatch, embedded_agentic_system_dispatch, embedded_artifact_dispatch,
    embedded_authorization_dispatch, embedded_budget_dispatch, embedded_council_dispatch,
    embedded_council_journal_dispatch, embedded_succession_dispatch,
};

// A child of the dispatcher that routes it, rather than another entry
// in the parent's list: the file next door is already at its budget,
// and a family of two belongs with the one thing that calls it.
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
