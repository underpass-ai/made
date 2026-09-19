//! Delegated lease heartbeat boundary validation.
use made_mcp_proto::v1 as pb;
use serde_json::Value;

pub(in crate::grpc) fn build_renew_step_lease_request(
    value: &Value,
) -> Result<pb::RenewCeremonyStepLeaseRequest, String> {
    let obj = value.as_object().ok_or("arguments must be an object")?;
    let text = |key: &str| {
        obj.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| format!("{key} must be a nonempty string"))
    };
    Ok(pb::RenewCeremonyStepLeaseRequest {
        ceremony_id: text("ceremony_id")?,
        step_id: text("step_id")?,
        claim_fence: text("claim_fence")?,
        lease_owner_id: text("lease_owner_id")?,
        renewal_id: text("renewal_id")?,
        lease_ttl_ms: obj
            .get("lease_ttl_ms")
            .and_then(Value::as_u64)
            .filter(|ttl| *ttl > 0)
            .ok_or("lease_ttl_ms must be a positive integer")?,
    })
}
