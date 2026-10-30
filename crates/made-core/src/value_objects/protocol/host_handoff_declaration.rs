use super::{HostWorkState, IdempotencyKey, LeaseOwnerId, StepClaimFence, StepId};
use crate::value_objects::EvidenceReference;
use crate::value_objects::HostAgentIncarnation;
use crate::DomainError;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Evidence supplied by an authenticated host about one exact accepted producer.
/// Incarnation is the host's stable worker/task invocation identity, not a new fence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostHandoffDeclaration {
    pub id: IdempotencyKey,
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub owner: LeaseOwnerId,
    pub incarnation: HostAgentIncarnation,
    pub state: HostWorkState,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub evidence: EvidenceReference,
}

impl HostHandoffDeclaration {
    pub fn validate(&self) -> Result<(), DomainError> {
        for (field, value, max) in [
            ("host_handoff.id", self.id.as_str(), 256),
            ("host_handoff.owner", self.owner.as_str(), 256),
            ("host_handoff.evidence", self.evidence.as_str(), 2048),
        ] {
            if value.trim().is_empty() {
                return Err(DomainError::EmptyField { field });
            }
            if value.chars().count() > max {
                return Err(DomainError::FieldTooLong {
                    field,
                    max,
                    actual: value.chars().count(),
                });
            }
            if value.chars().any(char::is_control) {
                return Err(DomainError::InvalidCharacters { field });
            }
        }
        Ok(())
    }
}
