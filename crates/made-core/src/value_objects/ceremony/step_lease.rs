use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::error::DomainError;
use crate::value_objects::DurationMs;

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

    /// Acquire a lease that expires `ttl` after `acquired_at`.
    ///
    /// Unlike [`StepLease::new`], which takes an already-computed expiry
    /// instant, this constructor derives the expiry from a typed
    /// [`DurationMs`] and **fails fast** with [`DomainError::OutOfRange`]
    /// when the requested lifetime cannot be honoured — either because it
    /// exceeds the signed-millisecond range the clock accepts or because
    /// adding it to `acquired_at` overflows the representable calendar.
    /// The requested TTL is never silently clamped.
    pub fn acquire(
        owner_id: LeaseOwnerId,
        idempotency_key: IdempotencyKey,
        acquired_at: OffsetDateTime,
        ttl: DurationMs,
    ) -> Result<Self, DomainError> {
        let ttl_millis = i64::try_from(ttl.get()).map_err(|_| DomainError::OutOfRange {
            field: "step_lease.ttl_ms",
            value: ttl.get() as f64,
            min: 0.0,
            max: i64::MAX as f64,
        })?;
        let expires_at = acquired_at
            .checked_add(Duration::milliseconds(ttl_millis))
            .ok_or(DomainError::OutOfRange {
                field: "step_lease.expires_at",
                value: ttl.get() as f64,
                min: 0.0,
                max: i64::MAX as f64,
            })?;
        Self::new(owner_id, idempotency_key, acquired_at, expires_at)
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

#[cfg(test)]
mod tests {
    use time::{Duration, OffsetDateTime};

    use super::*;

    fn owner() -> LeaseOwnerId {
        LeaseOwnerId::new("runner-1").unwrap()
    }

    fn key() -> IdempotencyKey {
        IdempotencyKey::new("ceremony-1:open_room:1").unwrap()
    }

    #[test]
    fn acquire_expires_ttl_after_acquired_at() {
        let acquired_at = OffsetDateTime::UNIX_EPOCH;
        let lease =
            StepLease::acquire(owner(), key(), acquired_at, DurationMs::from_millis(60_000))
                .unwrap();

        assert_eq!(lease.acquired_at(), acquired_at);
        assert_eq!(
            lease.expires_at(),
            acquired_at + Duration::milliseconds(60_000)
        );
        assert_eq!(lease.owner_id(), &owner());
        assert_eq!(lease.idempotency_key(), &key());
    }

    #[test]
    fn acquire_lease_is_not_expired_before_ttl_elapses() {
        let acquired_at = OffsetDateTime::UNIX_EPOCH;
        let lease = StepLease::acquire(owner(), key(), acquired_at, DurationMs::from_millis(1_000))
            .unwrap();

        assert!(!lease.is_expired_at(acquired_at + Duration::milliseconds(999)));
        assert!(lease.is_expired_at(acquired_at + Duration::milliseconds(1_000)));
    }

    #[test]
    fn acquire_rejects_zero_ttl_as_non_positive_lifetime() {
        let err = StepLease::acquire(owner(), key(), OffsetDateTime::UNIX_EPOCH, DurationMs::ZERO)
            .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }

    #[test]
    fn acquire_rejects_ttl_exceeding_signed_millisecond_range() {
        let err = StepLease::acquire(
            owner(),
            key(),
            OffsetDateTime::UNIX_EPOCH,
            DurationMs::from_millis(u64::MAX),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::OutOfRange {
                field: "step_lease.ttl_ms",
                ..
            }
        ));
    }

    #[test]
    fn acquire_rejects_ttl_that_overflows_the_calendar() {
        let err = StepLease::acquire(
            owner(),
            key(),
            OffsetDateTime::UNIX_EPOCH,
            DurationMs::from_millis(i64::MAX as u64),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::OutOfRange {
                field: "step_lease.expires_at",
                ..
            }
        ));
    }

    #[test]
    fn new_rejects_expiry_not_after_acquired_at() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let err = StepLease::new(owner(), key(), now, now).unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }
}
