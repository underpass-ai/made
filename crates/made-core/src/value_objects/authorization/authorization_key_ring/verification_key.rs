use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{VerificationKeyFingerprint, VerificationKeyId};
use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationKey {
    id: VerificationKeyId,
    fingerprint: VerificationKeyFingerprint,
    not_before: OffsetDateTime,
    expires_at: Option<OffsetDateTime>,
}

impl VerificationKey {
    pub fn new(
        id: VerificationKeyId,
        fingerprint: VerificationKeyFingerprint,
        not_before: OffsetDateTime,
        expires_at: Option<OffsetDateTime>,
    ) -> Result<Self, DomainError> {
        if expires_at.is_some_and(|value| value <= not_before) {
            return Err(DomainError::InvariantViolated {
                reason: "verification key expiry must follow activation",
            });
        }
        Ok(Self {
            id,
            fingerprint,
            not_before,
            expires_at,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &VerificationKeyId {
        &self.id
    }

    #[must_use]
    pub const fn fingerprint(&self) -> &VerificationKeyFingerprint {
        &self.fingerprint
    }

    #[must_use]
    pub fn accepts_at(&self, at: OffsetDateTime) -> bool {
        at >= self.not_before && self.expires_at.is_none_or(|expires| at < expires)
    }
}
