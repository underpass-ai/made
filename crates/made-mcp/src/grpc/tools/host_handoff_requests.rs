use super::super::json_to_proto as j2p;
use made_mcp_proto::v1 as pb;
use serde_json::Value;

pub(super) fn record(arguments: &Value) -> Result<pb::RecordCeremonyHostHandoffRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    let d = j2p::require_object(
        obj.get("declaration").ok_or("missing declaration")?,
        "declaration",
    )?;
    Ok(pb::RecordCeremonyHostHandoffRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.into(),
        declaration: Some(pb::HostHandoffDeclaration {
            id: j2p::require_str(d, "id")?.into(),
            step_id: j2p::require_str(d, "step_id")?.into(),
            claim_fence: j2p::require_str(d, "claim_fence")?.into(),
            owner: j2p::require_str(d, "owner")?.into(),
            incarnation: j2p::require_str(d, "incarnation")?.into(),
            state: j2p::require_str(d, "state")?.into(),
            observed_at: j2p::require_str(d, "observed_at")?.into(),
            evidence: j2p::require_str(d, "evidence")?.into(),
        }),
    })
}

pub(super) fn inspect(arguments: &Value) -> Result<pb::InspectCeremonyResumeRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    let limit = obj.get("limit").and_then(Value::as_u64).unwrap_or(100);
    Ok(pb::InspectCeremonyResumeRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.into(),
        after_claim: j2p::optional_str(obj, "after_claim")
            .unwrap_or_default()
            .into(),
        limit: u32::try_from(limit).map_err(|_| "invalid limit")?,
    })
}
