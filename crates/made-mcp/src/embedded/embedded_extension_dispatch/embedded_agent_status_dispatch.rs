use made_core::entities::{
    AgentExecutionStatus, AgentLiveness, AgentStatusSource, AgentUsageKind, CeremonyAgentStatus,
};
use made_core::ports::CeremonyAgentStatusQuery;
use made_core::value_objects::{
    CeremonyAgentExecutionId, CeremonyId, ExecutionOperationId, HostAgentIncarnation, LeaseOwnerId,
    LogicalWorkerId, RoleId, StepClaimFence, StepId,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::protocol::{tool_success_result, ToolError};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_list_ceremony_agents"
            | "made_get_ceremony_agent"
            | "made_report_ceremony_agent_status"
    )
}

pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let object = arguments
        .as_object()
        .ok_or_else(|| ToolError::invalid_request("arguments must be an object"))?;
    let value = match name {
        "made_list_ceremony_agents" => {
            let query = CeremonyAgentStatusQuery::new(
                required(object, "ceremony_id")?,
                optional(object, "cursor"),
                object.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize,
                object
                    .get("execution_status")
                    .map(parse_execution_status)
                    .transpose()?,
            )?;
            let page = made.list_agents(query).await?;
            json!({"agents": page.entries().iter().map(present).collect::<Vec<_>>(), "next_cursor": page.next_cursor()})
        }
        "made_get_ceremony_agent" => present(
            &made
                .get_agent(
                    required(object, "ceremony_id")?,
                    required(object, "agent_execution_id")?,
                )
                .await?,
        ),
        "made_report_ceremony_agent_status" => {
            let status =
                parse_status(object.get("status").ok_or_else(|| {
                    ToolError::invalid_request("missing required object `status`")
                })?)?;
            present(&made.report_agent_status(status).await?)
        }
        _ => unreachable!(),
    };
    Ok(tool_success_result(value))
}

fn required<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, ToolError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ToolError::invalid_request(format!("missing required string `{key}`")))
}
fn optional(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_owned)
}
fn parse_execution_status(value: &Value) -> Result<AgentExecutionStatus, ToolError> {
    serde_json::from_value(value.clone())
        .map_err(|_| ToolError::invalid_request("unsupported execution_status"))
}
fn parse_liveness(value: &Value) -> Result<AgentLiveness, ToolError> {
    serde_json::from_value(value.clone())
        .map_err(|_| ToolError::invalid_request("unsupported liveness"))
}
fn parse_source(value: &Value) -> Result<AgentStatusSource, ToolError> {
    serde_json::from_value(value.clone())
        .map_err(|_| ToolError::invalid_request("unsupported source"))
}
fn optional_string(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    optional(object, key)
}
fn parse_status(value: &Value) -> Result<CeremonyAgentStatus, ToolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ToolError::invalid_request("status must be an object"))?;
    let observed_at = OffsetDateTime::parse(required(object, "observed_at")?, &Rfc3339)
        .map_err(|_| ToolError::invalid_request("observed_at must be RFC3339"))?;
    CeremonyAgentStatus::new(
        CeremonyId::new(required(object, "ceremony_id")?)?,
        CeremonyAgentExecutionId::new(required(object, "agent_execution_id")?)?,
        ExecutionOperationId::new(required(object, "operation_id")?)?,
        LeaseOwnerId::new(required(object, "claim_owner_id")?)?,
        LogicalWorkerId::new(required(object, "logical_worker_id")?)?,
        LeaseOwnerId::new(required(object, "host_agent_id")?)?,
        HostAgentIncarnation::new(required(object, "host_agent_incarnation")?)?,
        optional_string(object, "previous_host_agent_id")
            .map(LeaseOwnerId::new)
            .transpose()?,
        optional_string(object, "previous_host_agent_incarnation")
            .map(HostAgentIncarnation::new)
            .transpose()?,
        RoleId::new(required(object, "role_id")?)?,
        StepId::new(required(object, "step_id")?)?,
        object.get("attempt").and_then(Value::as_u64).unwrap_or(0) as u32,
        parse_execution_status(
            object
                .get("execution_status")
                .ok_or_else(|| ToolError::invalid_request("missing execution_status"))?,
        )?,
        parse_liveness(
            object
                .get("liveness")
                .ok_or_else(|| ToolError::invalid_request("missing liveness"))?,
        )?,
        parse_source(
            object
                .get("source")
                .ok_or_else(|| ToolError::invalid_request("missing source"))?,
        )?,
        optional_string(object, "requested_model"),
        optional_string(object, "requested_reasoning_effort"),
        optional_string(object, "actual_model"),
        optional_string(object, "actual_reasoning_effort"),
        required(object, "activity")?,
        optional_string(object, "blocker"),
        optional_string(object, "dependency"),
        required(object, "task_summary")?,
        object
            .get("evidence_references")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        optional_string(object, "usage_kind")
            .as_deref()
            .map(AgentUsageKind::try_from)
            .transpose()?,
        object.get("usage_value").and_then(Value::as_u64),
        observed_at,
        object
            .get("report_sequence")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        required(object, "idempotency_key")?,
        StepClaimFence::new(required(object, "claim_fence")?)?,
    )
    .map_err(Into::into)
}

fn present(status: &CeremonyAgentStatus) -> Value {
    json!({"ceremony_id": status.ceremony_id(), "agent_execution_id": status.agent_execution_id(), "operation_id": status.operation_id().as_str(), "claim_owner_id": status.claim_owner_id().as_str(), "logical_worker_id": status.logical_worker_id(), "host_agent_id": status.host_agent_id(), "host_agent_incarnation": status.host_agent_incarnation(), "previous_host_agent_id": status.previous_host_agent_id(), "previous_host_agent_incarnation": status.previous_host_agent_incarnation(), "role_id": status.role_id(), "step_id": status.step_id(), "attempt": status.attempt(), "execution_status": serde_json::to_value(status.execution_status()).unwrap(), "liveness": serde_json::to_value(status.liveness()).unwrap(), "source": serde_json::to_value(status.source()).unwrap(), "requested_model": status.requested_model(), "requested_reasoning_effort": status.requested_reasoning_effort(), "actual_model": status.actual_model(), "actual_reasoning_effort": status.actual_reasoning_effort(), "activity": status.activity(), "blocker": status.blocker(), "dependency": status.dependency(), "task_summary": status.task_summary(), "evidence_references": status.evidence_references(), "usage_kind": status.usage_kind(), "usage_value": status.usage_value(), "observed_at": status.observed_at().format(&Rfc3339).unwrap(), "report_sequence": status.report_sequence(), "idempotency_key": status.idempotency_key(), "claim_fence": status.claim_fence().as_str()})
}
