use super::embedded_request_fields::required_string;
use made_app::workers::RenewCeremonyStepLeaseInput;
use made_core::value_objects::{
    CeremonyId, DurationMs, IdempotencyKey, LeaseOwnerId, StepClaimFence, StepId,
    StepLeaseRenewalRequest,
};
use serde_json::{json, Value};

pub(super) fn parse(value: &Value) -> Result<RenewCeremonyStepLeaseInput, String> {
    let object = value.as_object().ok_or("arguments must be an object")?;
    let ttl = object
        .get("lease_ttl_ms")
        .and_then(Value::as_u64)
        .filter(|ttl| *ttl > 0)
        .ok_or("lease_ttl_ms must be a positive integer")?;
    let parsed = || -> Result<_, made_core::DomainError> {
        Ok(RenewCeremonyStepLeaseInput {
            ceremony_id: CeremonyId::new(
                required_string(object, "ceremony_id").unwrap_or_default(),
            )?,
            step_id: StepId::new(required_string(object, "step_id").unwrap_or_default())?,
            claim_fence: StepClaimFence::new(
                required_string(object, "claim_fence").unwrap_or_default(),
            )?,
            owner: LeaseOwnerId::new(
                required_string(object, "lease_owner_id").unwrap_or_default(),
            )?,
            request: StepLeaseRenewalRequest {
                id: IdempotencyKey::new(required_string(object, "renewal_id").unwrap_or_default())?,
                ttl: DurationMs::from_millis(ttl),
            },
        })
    };
    parsed().map_err(|error| error.to_string())
}

pub(super) fn present(receipt: made_core::entities::ceremony_events::StepLeaseRenewed) -> Value {
    let moment = |at: time::OffsetDateTime| {
        at.format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default()
    };
    json!({
        "renewal_id": receipt.request.expect("public renewal receipt").id.as_str(),
        "claim_fence": receipt.claim_fence.as_str(),
        "effective_lease_expires_at": moment(receipt.expires_at),
        "renewed_at": moment(receipt.renewed_at),
    })
}
