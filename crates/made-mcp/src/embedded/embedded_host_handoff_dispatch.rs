use crate::mcp_protocol::{tool_success_result, ToolError};
use made_app::workers::{InspectProtocolResumeInput, RecordProtocolHostHandoffInput};
use made_core::value_objects::{
    ExecutionRecoveryPageLimit, HostHandoffDeclaration, ProtocolId, StepClaimFence,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

pub(super) async fn record(made: &EmbeddedMade, arguments: &Value) -> Result<Value, ToolError> {
    let protocol_id = protocol_id(arguments)?;
    let declaration: HostHandoffDeclaration = serde_json::from_value(
        arguments
            .get("declaration")
            .cloned()
            .ok_or_else(|| ToolError::invalid_request("missing declaration"))?,
    )
    .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    declaration.validate().map_err(domain_tool_error)?;
    let result = made
        .record_protocol_host_handoff(RecordProtocolHostHandoffInput {
            protocol_id,
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
    let input = InspectProtocolResumeInput {
        protocol_id: protocol_id(arguments)?,
        after_claim,
        limit: ExecutionRecoveryPageLimit::new(limit).map_err(domain_tool_error)?,
    };
    let result = made
        .inspect_protocol_resume(input)
        .await
        .map_err(domain_tool_error)?;
    serde_json::to_value(result)
        .map(tool_success_result)
        .map_err(|error| ToolError::refused(error.to_string()))
}

fn protocol_id(arguments: &Value) -> Result<ProtocolId, ToolError> {
    ProtocolId::new(
        arguments
            .get("protocol_id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
    .map_err(domain_tool_error)
}

fn domain_tool_error(error: made_core::DomainError) -> ToolError {
    error.into()
}
