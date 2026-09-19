use made_core::value_objects::AuthorizationEvidence;
use made_core::DomainError;
use made_proto::v1 as pb;
use time::format_description::well_known::Rfc3339;

use super::authorization::{authorization_action_to_proto, authorization_scope_to_proto};

pub(super) fn evidence_to_proto(
    evidence: &AuthorizationEvidence,
) -> Result<pb::CeremonyAuthorizationEvidence, DomainError> {
    Ok(pb::CeremonyAuthorizationEvidence {
        decision_id: evidence.decision_id().as_str().to_owned(),
        request_id: evidence.request_id().as_str().to_owned(),
        principal_id: evidence.principal_id().as_str().to_owned(),
        action: authorization_action_to_proto(evidence.action()),
        scope: Some(authorization_scope_to_proto(evidence.scope())),
        target_digest: evidence.target_digest().as_str().to_owned(),
        policy_version: evidence.policy_version().value(),
        admitted_at: evidence
            .admitted_at()
            .format(&Rfc3339)
            .map_err(timestamp_error)?,
        valid_until: evidence
            .valid_until()
            .format(&Rfc3339)
            .map_err(timestamp_error)?,
    })
}

fn timestamp_error(_: time::error::Format) -> DomainError {
    DomainError::InvariantViolated {
        reason: "sealed authorization timestamp cannot be rendered canonically",
    }
}
