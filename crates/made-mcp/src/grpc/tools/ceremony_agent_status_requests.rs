use made_mcp_proto::v1 as pb;
use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::super::json_to_proto as j2p;

pub(super) fn list(args: &Value) -> Result<pb::ListCeremonyAgentsRequest, String> {
    let object = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::ListCeremonyAgentsRequest {
        ceremony_id: j2p::require_str(object, "ceremony_id")?.to_owned(),
        execution_status: j2p::optional_str(object, "execution_status").map(str::to_owned),
        cursor: j2p::optional_str(object, "cursor").map(str::to_owned),
        limit: j2p::optional_u32(object, "limit")?,
    })
}

pub(super) fn get(args: &Value) -> Result<pb::GetCeremonyAgentRequest, String> {
    let object = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::GetCeremonyAgentRequest {
        ceremony_id: j2p::require_str(object, "ceremony_id")?.to_owned(),
        agent_execution_id: j2p::require_str(object, "agent_execution_id")?.to_owned(),
    })
}

pub(super) fn report(args: &Value) -> Result<pb::ReportCeremonyAgentStatusRequest, String> {
    let object = j2p::require_object(args, "tools/call.arguments")?;
    let status = j2p::require_object(
        object
            .get("status")
            .ok_or("missing required object `status`")?,
        "status",
    )?;
    Ok(pb::ReportCeremonyAgentStatusRequest {
        status: Some(status_from_json(status)?),
    })
}

fn status_from_json(object: &Map<String, Value>) -> Result<pb::CeremonyAgentStatus, String> {
    let observed_at = OffsetDateTime::parse(j2p::require_str(object, "observed_at")?, &Rfc3339)
        .map_err(|_| "`observed_at` must be RFC3339".to_owned())?;
    Ok(pb::CeremonyAgentStatus {
        ceremony_id: j2p::require_str(object, "ceremony_id")?.to_owned(),
        agent_execution_id: j2p::require_str(object, "agent_execution_id")?.to_owned(),
        operation_id: j2p::require_str(object, "operation_id")?.to_owned(),
        claim_owner_id: j2p::require_str(object, "claim_owner_id")?.to_owned(),
        logical_worker_id: j2p::require_str(object, "logical_worker_id")?.to_owned(),
        host_agent_id: j2p::require_str(object, "host_agent_id")?.to_owned(),
        host_agent_incarnation: j2p::require_str(object, "host_agent_incarnation")?.to_owned(),
        previous_host_agent_id: j2p::optional_str(object, "previous_host_agent_id")
            .map(str::to_owned),
        previous_host_agent_incarnation: j2p::optional_str(
            object,
            "previous_host_agent_incarnation",
        )
        .map(str::to_owned),
        role_id: j2p::require_str(object, "role_id")?.to_owned(),
        step_id: j2p::require_str(object, "step_id")?.to_owned(),
        attempt: j2p::optional_u32(object, "attempt")?,
        execution_status: j2p::require_str(object, "execution_status")?.to_owned(),
        liveness: j2p::require_str(object, "liveness")?.to_owned(),
        source: j2p::require_str(object, "source")?.to_owned(),
        requested_model: j2p::optional_str(object, "requested_model").map(str::to_owned),
        requested_reasoning_effort: j2p::optional_str(object, "requested_reasoning_effort")
            .map(str::to_owned),
        actual_model: j2p::optional_str(object, "actual_model").map(str::to_owned),
        actual_reasoning_effort: j2p::optional_str(object, "actual_reasoning_effort")
            .map(str::to_owned),
        activity: j2p::require_str(object, "activity")?.to_owned(),
        blocker: j2p::optional_str(object, "blocker").map(str::to_owned),
        dependency: j2p::optional_str(object, "dependency").map(str::to_owned),
        task_summary: j2p::require_str(object, "task_summary")?.to_owned(),
        evidence_references: object
            .get("evidence_references")
            .map_or(Ok(Vec::new()), |value| {
                value
                    .as_array()
                    .ok_or("`evidence_references` must be an array".to_owned())
                    .and_then(|values| {
                        values
                            .iter()
                            .map(|value| {
                                value
                                    .as_str()
                                    .map(str::to_owned)
                                    .ok_or("evidence reference must be a string".to_owned())
                            })
                            .collect()
                    })
            })?,
        usage_kind: j2p::optional_str(object, "usage_kind").map(str::to_owned),
        usage_value: j2p::optional_present_u64(object, "usage_value")?,
        observed_at: Some(prost_types::Timestamp {
            seconds: observed_at.unix_timestamp(),
            nanos: observed_at.nanosecond() as i32,
        }),
        report_sequence: j2p::optional_u64(object, "report_sequence")?,
        idempotency_key: j2p::require_str(object, "idempotency_key")?.to_owned(),
        claim_fence: j2p::require_str(object, "claim_fence")?.to_owned(),
    })
}
