use made_mcp_proto::v1 as pb;
use serde_json::Value;

use super::super::json_to_proto as j2p;

pub(super) fn pull(arguments: &Value) -> Result<pb::PullCeremonyAgentInterventionsRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::PullCeremonyAgentInterventionsRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.into(),
        agent_execution_id: j2p::require_str(obj, "agent_execution_id")?.into(),
        incarnation: j2p::require_str(obj, "incarnation")?.into(),
        role_id: j2p::require_str(obj, "role_id")?.into(),
        lease_duration_ms: obj
            .get("lease_duration_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        limit: limit(obj, "limit")?,
    })
}

pub(super) fn acknowledge(
    arguments: &Value,
) -> Result<pb::AcknowledgeCeremonyAgentInterventionRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::AcknowledgeCeremonyAgentInterventionRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.into(),
        intervention_id: j2p::require_str(obj, "intervention_id")?.into(),
        delivery_id: j2p::require_str(obj, "delivery_id")?.into(),
        lease_id: j2p::require_str(obj, "lease_id")?.into(),
        agent_execution_id: j2p::require_str(obj, "agent_execution_id")?.into(),
        incarnation: j2p::require_str(obj, "incarnation")?.into(),
        role_id: j2p::require_str(obj, "role_id")?.into(),
        observation_kind: j2p::require_str(obj, "observation_kind")?.into(),
        note: j2p::optional_str(obj, "note").unwrap_or_default().into(),
        evidence: j2p::optional_str(obj, "evidence")
            .unwrap_or_default()
            .into(),
        observed_at: j2p::optional_str(obj, "observed_at")
            .unwrap_or_default()
            .into(),
    })
}

pub(super) fn get(arguments: &Value) -> Result<pb::GetCeremonyInterventionRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::GetCeremonyInterventionRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.into(),
        intervention_id: j2p::require_str(obj, "intervention_id")?.into(),
    })
}

pub(super) fn list(arguments: &Value) -> Result<pb::ListCeremonyInterventionsRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::ListCeremonyInterventionsRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.into(),
        status: j2p::optional_str(obj, "status").unwrap_or_default().into(),
        role_id: j2p::optional_str(obj, "role_id").unwrap_or_default().into(),
        agent_execution_id: j2p::optional_str(obj, "agent_execution_id")
            .unwrap_or_default()
            .into(),
        unresolved_only: obj
            .get("unresolved_only")
            .and_then(Value::as_bool)
            .unwrap_or_default(),
        limit: limit(obj, "limit")?,
        cursor: j2p::optional_str(obj, "cursor").unwrap_or_default().into(),
    })
}

/// Zero means "the server's own maximum", which is what the schema says.
fn limit(obj: &serde_json::Map<String, Value>, key: &str) -> Result<u32, String> {
    let raw = obj.get(key).and_then(Value::as_u64).unwrap_or_default();
    u32::try_from(raw).map_err(|_| format!("invalid {key}"))
}
