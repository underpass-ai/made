use crate::protocol::{tool_success_result, ToolError};
use made_embedded::EmbeddedMade;
use serde_json::Value;

mod presenter;
mod request;

pub(super) use presenter::{plan_value, source};

const TOOLS: [&str; 2] = [
    "made_plan_ceremony_successor",
    "made_start_ceremony_successor",
];

pub(super) fn handles(name: &str) -> bool {
    TOOLS.contains(&name)
}

pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    match name {
        "made_plan_ceremony_successor" => {
            let view = made.plan_successor(request::plan(arguments)?).await?;
            Ok(tool_success_result(presenter::plan_view(&view)))
        }
        "made_start_ceremony_successor" => {
            let outcome = made.start_successor(request::start(arguments)?).await?;
            Ok(tool_success_result(presenter::started(
                outcome.successor.id(),
                &outcome.plan,
            )))
        }
        other => Err(ToolError::invalid_request(format!(
            "unknown succession tool `{other}`"
        ))),
    }
}
