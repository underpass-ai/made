use std::sync::Arc;

use made_core::error::DomainError;
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::protocol::{tool_success_result, ToolError};

use super::super::embedded_council_presenter as present;
use super::super::embedded_council_requests as request;
use super::super::embedded_deliberation_observer::EmbeddedDeliberationObserver;

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_deliberate"
            | "made_stream_deliberation"
            | "made_get_deliberation_result"
            | "made_orchestrate"
            | "made_process_trigger_event"
            | "made_run_council_decision"
            | "made_create_council"
            | "made_list_councils"
            | "made_delete_council"
            | "made_register_agent"
            | "made_unregister_agent"
            | "made_register_contract"
            | "made_list_contracts"
            | "made_delete_contract"
    )
}

#[allow(clippy::too_many_lines)]
pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let value = match name {
        "made_deliberate" => {
            let output = made
                .deliberate(request::task(arguments).map_err(ToolError::invalid_request)?)
                .await?;
            present::deliberate(&output)
        }
        "made_stream_deliberation" => stream(made, arguments).await?,
        "made_get_deliberation_result" => {
            let task_id = made_core::value_objects::TaskId::new(
                arguments
                    .as_object()
                    .and_then(|object| object.get("task_id"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ToolError::invalid_request("missing required string `task_id`")
                    })?,
            )?;
            match made.get_deliberation_result(&task_id).await {
                Ok(output) => json!({ "found": true, "result": present::deliberation(&output) }),
                Err(DomainError::NotFound { .. }) => {
                    json!({ "found": false, "result": Value::Null })
                }
                Err(error) => return Err(error.into()),
            }
        }
        "made_orchestrate" => {
            let task = request::task(arguments).map_err(ToolError::invalid_request)?;
            let options =
                request::execution_options(arguments).map_err(ToolError::invalid_request)?;
            present::orchestrate(&made.orchestrate(task, options).await?)
        }
        "made_process_trigger_event" => {
            let trigger = request::trigger(arguments).map_err(ToolError::invalid_request)?;
            let event_id = trigger.envelope().event_id().as_str().to_owned();
            let outcome = made.process_trigger_event(&trigger).await?;
            json!({
                "ack": {
                    "event_id": event_id,
                    "accepted": outcome.accepted(),
                    "dispatched_task_ids": outcome.dispatched_task_ids().iter().map(made_core::value_objects::TaskId::as_str).collect::<Vec<_>>(),
                    "reason": if outcome.accepted() { "" } else { "no specialties produced a deliberation" },
                }
            })
        }
        "made_run_council_decision" => {
            let input =
                request::run_council_decision(arguments).map_err(ToolError::invalid_request)?;
            present::run_council_decision(&made.run_council_decision(input).await?)
        }
        "made_create_council" => {
            let input = request::create_council(arguments).map_err(ToolError::invalid_request)?;
            json!({ "council": present::council(&made.create_council(input).await?) })
        }
        "made_list_councils" => json!({
            "councils": made.list_councils().await?.iter().map(present::council).collect::<Vec<_>>()
        }),
        "made_delete_council" => {
            let specialty = request::specialty(arguments).map_err(ToolError::invalid_request)?;
            json!({ "deleted": boolean_delete(made.delete_council(&specialty).await)? })
        }
        "made_register_agent" => {
            let descriptor = request::agent(arguments).map_err(ToolError::invalid_request)?;
            json!({ "agent_id": made.register_agent(descriptor).await?.as_str() })
        }
        "made_unregister_agent" => {
            let id = request::agent_id(arguments).map_err(ToolError::invalid_request)?;
            json!({ "unregistered": boolean_delete(made.unregister_agent(&id).await)? })
        }
        "made_register_contract" => {
            let contract = request::contract(arguments).map_err(ToolError::invalid_request)?;
            let id = contract.contract_id().as_str().to_owned();
            made.register_contract(contract).await?;
            json!({ "contract_id": id })
        }
        "made_list_contracts" => json!({
            "contracts": made.list_contracts().await?.iter().map(present::contract).collect::<Vec<_>>()
        }),
        "made_delete_contract" => {
            let id = request::contract_id(arguments).map_err(ToolError::invalid_request)?;
            json!({ "deleted": boolean_delete(made.delete_contract(&id).await)? })
        }
        _ => unreachable!("embedded council dispatch called for unsupported tool"),
    };
    Ok(tool_success_result(value))
}

async fn stream(made: &EmbeddedMade, arguments: &Value) -> Result<Value, ToolError> {
    let task = request::task(arguments).map_err(ToolError::invalid_request)?;
    let task_id = task.id().as_str().to_owned();
    let (sender, mut receiver) = mpsc::channel(16);
    let observer = Arc::new(EmbeddedDeliberationObserver::new(sender));
    let deliberation = made.stream_deliberation(task, observer);
    tokio::pin!(deliberation);
    let mut frames = Vec::new();
    let output = loop {
        tokio::select! {
            result = &mut deliberation => {
                break result.map_err(|error| {
                    ToolError::refused(format!("stream item failed: {error}"))
                })?
            },
            Some(frame) = receiver.recv() => frames.push(frame),
        }
    };
    while let Ok(frame) = receiver.try_recv() {
        frames.push(frame);
    }
    let final_frame = present::result_frame(&output);
    let winner = final_frame["payload"]["result"].clone();
    frames.push(final_frame);
    Ok(json!({ "task_id": task_id, "frames": frames, "winner": winner }))
}

fn boolean_delete(result: Result<(), DomainError>) -> Result<bool, ToolError> {
    match result {
        Ok(()) => Ok(true),
        Err(DomainError::NotFound { .. }) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::*;
    use crate::protocol::ToolErrorCode;

    #[tokio::test]
    async fn an_early_stream_failure_returns_without_leaving_background_work() {
        let made = EmbeddedMade::builder().build();
        let error = tokio::time::timeout(
            Duration::from_millis(100),
            stream(
                &made,
                &json!({
                    "task": {
                        "task_id": "missing-council",
                        "description": "fail before the first phase",
                        "specialty": "unknown",
                    },
                }),
            ),
        )
        .await
        .expect("an early engine error must finish the call-scoped future")
        .expect_err("the unknown council must fail");

        assert_eq!(error.code(), ToolErrorCode::Refused);
    }
}
