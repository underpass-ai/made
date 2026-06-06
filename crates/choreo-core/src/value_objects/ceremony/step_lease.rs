use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::DomainError;

use super::{IdempotencyKey, LeaseOwnerId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepLease {
    owner_id: LeaseOwnerId,
    idempotency_key: IdempotencyKey,
    #[serde(with = "time::serde::rfc3339")]
    acquired_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

impl StepLease {
    pub fn new(
        owner_id: LeaseOwnerId,
        idempotency_key: IdempotencyKey,
        acquired_at: OffsetDateTime,
        expires_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        if expires_at <= acquired_at {
            return Err(DomainError::InvariantViolated {
                reason: "step lease must expire after it is acquired",
            });
        }
        Ok(Self {
            owner_id,
            idempotency_key,
            acquired_at,
            expires_at,
        })
    }

    #[must_use]
    pub fn owner_id(&self) -> &LeaseOwnerId {
        &self.owner_id
    }

    #[must_use]
    pub fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }

    #[must_use]
    pub fn acquired_at(&self) -> OffsetDateTime {
        self.acquired_at
    }

    #[must_use]
    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }

    #[must_use]
    pub fn is_expired_at(&self, now: OffsetDateTime) -> bool {
        now >= self.expires_at
    }
}
