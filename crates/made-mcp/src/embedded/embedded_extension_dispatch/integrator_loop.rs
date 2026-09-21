//! The five tools of the integrator loop, over the in-process engine.

use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::integrator_loop_presenter::{
    present_acknowledged, present_batch, present_bind, present_binding_answer, present_page,
};
use super::integrator_loop_requests as requests;
use crate::protocol::{tool_success_result, ToolError};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_bind_ceremony_integrator"
            | "made_get_ceremony_integrator_binding"
            | "made_await_integrator_attention"
            | "made_acknowledge_integrator_attention"
            | "made_list_attention_deliveries"
    )
}

pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let object = arguments
        .as_object()
        .ok_or_else(|| ToolError::invalid_request("tools/call.arguments must be an object"))?;
    let value = match name {
        "made_bind_ceremony_integrator" => {
            let outcome = made.bind_integrator(requests::bind(object)?).await?;
            present_bind(&outcome)
        }
        "made_get_ceremony_integrator_binding" => {
            let scope = requests::binding_scope(object)?;
            let binding = made.get_integrator_binding(&scope).await?;
            present_binding_answer(binding.as_ref())
        }
        "made_await_integrator_attention" => {
            let batch = made
                .await_integrator_attention(requests::await_attention(object)?)
                .await?;
            present_batch(&batch)
        }
        "made_acknowledge_integrator_attention" => {
            let acknowledged = made
                .acknowledge_integrator_attention(requests::acknowledge(object)?)
                .await?;
            present_acknowledged(&acknowledged)
        }
        "made_list_attention_deliveries" => {
            let page = made
                .list_attention_deliveries(requests::deliveries(object)?)
                .await?;
            present_page(
                page.records(),
                page.next_cursor().map(|cursor| cursor.as_str()),
            )
        }
        _ => unreachable!("the dispatcher only routes what it handles"),
    };
    Ok(tool_success_result(value))
}
