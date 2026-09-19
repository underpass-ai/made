use crate::protocol::{tool_success_result, ToolError};
use made_app::workers::{InspectCeremonyResumeInput, RecordCeremonyHostHandoffInput};
use made_core::value_objects::{
    CeremonyId, ExecutionRecoveryPageLimit, HostHandoffDeclaration, StepClaimFence,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

pub(super) async fn record(made: &EmbeddedMade, arguments: &Value) -> Result<Value, ToolError> {
    let ceremony_id = ceremony_id(arguments)?;
    let declaration: HostHandoffDeclaration = serde_json::from_value(
        arguments
            .get("declaration")
            .cloned()
            .ok_or_else(|| ToolError::invalid_request("missing declaration"))?,
    )
    .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    declaration.validate().map_err(domain_tool_error)?;
    let result = made
        .record_ceremony_host_handoff(RecordCeremonyHostHandoffInput {
            ceremony_id,
            declaration,
        })
        .await
        .map_err(domain_tool_error)?;
    serde_json::to_value(result)
        .map(tool_success_result)
        .map_err(|error| ToolError::refused(error.to_string()))
}

pub(super) async fn inspect(made: &EmbeddedMade, arguments: &Value) -> Result<Value, ToolError> {
    let after_claim = arguments
        .get("after_claim")
        .and_then(Value::as_str)
        .map(StepClaimFence::new)
        .transpose()
        .map_err(domain_tool_error)?;
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(100);
    let limit = u16::try_from(limit).map_err(|_| ToolError::invalid_request("invalid limit"))?;
    let input = InspectCeremonyResumeInput {
        ceremony_id: ceremony_id(arguments)?,
        after_claim,
        limit: ExecutionRecoveryPageLimit::new(limit).map_err(domain_tool_error)?,
    };
    let result = made
        .inspect_ceremony_resume(input)
        .await
        .map_err(domain_tool_error)?;
    serde_json::to_value(result)
        .map(tool_success_result)
        .map_err(|error| ToolError::refused(error.to_string()))
}

fn ceremony_id(arguments: &Value) -> Result<CeremonyId, ToolError> {
    CeremonyId::new(
        arguments
            .get("ceremony_id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
    .map_err(domain_tool_error)
}

fn domain_tool_error(error: made_core::DomainError) -> ToolError {
    error.into()
}
