use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{CeremonyId, StepExecutionRecord, StepId};
use crate::error::DomainError;

/// Identity of exactly one accepted claim. It is a concurrency fence, not a credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StepClaimFence(String);

impl StepClaimFence {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(DomainError::InvalidCharacters {
                field: "claim_fence",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn for_record(
        instance_id: &CeremonyId,
        step_id: &StepId,
        record: &StepExecutionRecord,
    ) -> Result<Self, DomainError> {
        let lease = record.lease().ok_or(DomainError::InvariantViolated {
            reason: "claim fence requires a step lease",
        })?;
        // Domain-separated, length-delimited fields avoid concatenation ambiguity.
        // Derived from existing sealed data; no event or snapshot bytes change.
        let mut digest = Sha256::new();
        digest.update(b"made.step-claim-fence.v1\0");
        for part in [
            instance_id.as_str().to_owned(),
            step_id.as_str().to_owned(),
            record.state_visit().get().to_string(),
            record.state_iteration().get().to_string(),
            record.iteration().get().to_string(),
            record.attempt().get().to_string(),
            lease.owner_id().as_str().to_owned(),
            lease.idempotency_key().as_str().to_owned(),
            lease.acquired_at().unix_timestamp_nanos().to_string(),
            lease.expires_at().unix_timestamp_nanos().to_string(),
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part.as_bytes());
        }
        Ok(Self(format!("{:x}", digest.finalize())))
    }
}

impl TryFrom<String> for StepClaimFence {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<StepClaimFence> for String {
    fn from(value: StepClaimFence) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{
        DurationMs, IdempotencyKey, LeaseOwnerId, StateIteration, StateVisit, StepAttempt,
        StepIteration, StepLease,
    };
    use time::OffsetDateTime;

    fn record(key: &str) -> StepExecutionRecord {
        StepExecutionRecord::pending().with_started(
            StepLease::acquire(
                LeaseOwnerId::new("host").unwrap(),
                IdempotencyKey::new(key).unwrap(),
                OffsetDateTime::UNIX_EPOCH,
                DurationMs::from_millis(1000),
            )
            .unwrap(),
            StepAttempt::FIRST,
            None,
        )
    }
    fn fence(instance: &str, step: &str, record: &StepExecutionRecord) -> StepClaimFence {
        StepClaimFence::for_record(
            &CeremonyId::new(instance).unwrap(),
            &StepId::new(step).unwrap(),
            record,
        )
        .unwrap()
    }
    #[test]
    fn parsed_fences_are_canonical_sha256_values() {
        for invalid in [
            String::new(),
            "0".repeat(63),
            "0".repeat(65),
            "A".repeat(64),
            "g".repeat(64),
        ] {
            assert!(StepClaimFence::new(invalid).is_err());
        }
        let original = fence("instance", "step", &record("claim"));
        assert_eq!(StepClaimFence::new(original.as_str()).unwrap(), original);
    }
    #[test]
    fn fence_changes_when_only_the_state_visit_changes() {
        let first = record("same-key");
        let reentered = first.clone().with_state_visit(StateVisit::new(3).unwrap());
        assert_eq!(first.lease(), reentered.lease());
        assert_eq!(first.state_iteration(), reentered.state_iteration());
        assert_eq!(first.iteration(), reentered.iteration());
        assert_eq!(first.attempt(), reentered.attempt());
        assert_ne!(
            fence("instance", "step", &first),
            fence("instance", "step", &reentered)
        );
    }

    #[test]
    fn fence_binds_scope_coordinates_and_complete_lease_without_ambiguous_concatenation() {
        let original_record = record("claim");
        let original = fence("ab", "c", &original_record);
        assert_ne!(original, fence("a", "bc", &original_record));
        assert_ne!(original, fence("other", "c", &original_record));
        assert_ne!(original, fence("ab", "other", &original_record));
        assert_ne!(original, fence("ab", "c", &record("replacement")));
        let lease = original_record.lease().unwrap().clone();
        for replacement in
            [
                StepExecutionRecord::pending().with_started(
                    lease.clone(),
                    StepAttempt::new(2).unwrap(),
                    None,
                ),
                StepExecutionRecord::pending_state_iteration(StateIteration::new(2).unwrap())
                    .with_started(lease.clone(), StepAttempt::FIRST, None),
                StepExecutionRecord::pending_iteration(StepIteration::new(2).unwrap())
                    .with_started(lease, StepAttempt::FIRST, None),
            ]
        {
            assert_ne!(original, fence("ab", "c", &replacement));
        }
        assert!(StepClaimFence::for_record(
            &CeremonyId::new("ab").unwrap(),
            &StepId::new("c").unwrap(),
            &StepExecutionRecord::pending()
        )
        .is_err());
    }
}
