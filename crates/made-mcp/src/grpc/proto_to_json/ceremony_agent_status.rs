use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

use super::timestamp_to_rfc3339;

pub(crate) fn status_to_json(status: &pb::CeremonyAgentStatus) -> Value {
    json!({
        "ceremony_id": status.ceremony_id, "agent_execution_id": status.agent_execution_id,
        "operation_id": status.operation_id, "claim_owner_id": status.claim_owner_id,
        "logical_worker_id": status.logical_worker_id, "host_agent_id": status.host_agent_id,
        "host_agent_incarnation": status.host_agent_incarnation,
        "previous_host_agent_id": status.previous_host_agent_id,
        "previous_host_agent_incarnation": status.previous_host_agent_incarnation,
        "role_id": status.role_id, "step_id": status.step_id, "attempt": status.attempt,
        "execution_status": status.execution_status, "liveness": status.liveness, "source": status.source,
        "requested_model": status.requested_model, "requested_reasoning_effort": status.requested_reasoning_effort,
        "actual_model": status.actual_model, "actual_reasoning_effort": status.actual_reasoning_effort,
        "activity": status.activity, "blocker": status.blocker, "dependency": status.dependency,
        "task_summary": status.task_summary, "evidence_references": status.evidence_references,
        "usage_kind": status.usage_kind, "usage_value": status.usage_value,
        "observed_at": timestamp_to_rfc3339(status.observed_at.as_ref()),
        "report_sequence": status.report_sequence, "idempotency_key": status.idempotency_key,
        "claim_fence": status.claim_fence,
    })
}
