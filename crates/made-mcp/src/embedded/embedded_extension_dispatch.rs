use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::protocol::{tool_success_result, ToolError};

use super::{
    embedded_agent_status_dispatch, embedded_agentic_system_dispatch, embedded_artifact_dispatch,
    embedded_authorization_dispatch, embedded_budget_dispatch, embedded_council_dispatch,
    embedded_council_journal_dispatch, embedded_succession_dispatch,
};

pub(super) fn handles(name: &str) -> bool {
    embedded_authorization_dispatch::handles(name)
        || embedded_council_journal_dispatch::handles(name)
        || embedded_budget_dispatch::handles(name)
        || embedded_council_dispatch::handles(name)
        || embedded_artifact_dispatch::handles(name)
        || embedded_agent_status_dispatch::handles(name)
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
