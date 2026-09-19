use std::time::Duration;

use made_core::error::DomainError;
use made_core::value_objects::DurationMs;

/// Bounds one continuous host turn and its retry sleep. The host remains
/// cooperative: it never creates a process or task that outlives the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyWorkerHostPolicy {
    max_pages_per_turn: u16,
    initial_backoff: DurationMs,
    maximum_backoff: DurationMs,
}

impl CeremonyWorkerHostPolicy {
    pub const MAX_PAGES_PER_TURN: u16 = 32;

    pub fn new(
        max_pages_per_turn: u16,
        initial_backoff: DurationMs,
        maximum_backoff: DurationMs,
    ) -> Result<Self, DomainError> {
        if max_pages_per_turn == 0 || max_pages_per_turn > Self::MAX_PAGES_PER_TURN {
            return Err(DomainError::OutOfRange {
                field: "worker_max_pages_per_turn",
                value: f64::from(max_pages_per_turn),
                min: 1.0,
                max: f64::from(Self::MAX_PAGES_PER_TURN),
            });
        }
        if maximum_backoff < initial_backoff {
            return Err(DomainError::InvariantViolated {
                reason: "worker maximum backoff must not be below initial backoff",
            });
        }
        Ok(Self {
            max_pages_per_turn,
            initial_backoff,
            maximum_backoff,
        })
    }

    #[must_use]
    pub const fn max_pages_per_turn(self) -> u16 {
        self.max_pages_per_turn
    }

    #[must_use]
    pub const fn initial_backoff(self) -> DurationMs {
        self.initial_backoff
    }

    #[must_use]
    pub const fn maximum_backoff(self) -> DurationMs {
        self.maximum_backoff
    }

    #[must_use]
    pub fn backoff(self, attempt: u32) -> Duration {
        let shift = attempt.saturating_sub(1).min(10);
        let initial = self.initial_backoff.get();
        let millis = initial
            .saturating_mul(1_u64 << shift)
            .min(self.maximum_backoff.get());
        Duration::from_millis(millis)
    }
}

impl Default for CeremonyWorkerHostPolicy {
    fn default() -> Self {
        Self::new(
            1,
            DurationMs::from_millis(100),
            DurationMs::from_millis(2_000),
        )
        .expect("default worker host policy is valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_bounded_and_zero_is_a_valid_test_policy() {
        let policy = CeremonyWorkerHostPolicy::new(
            2,
            DurationMs::from_millis(1),
            DurationMs::from_millis(4),
        )
        .unwrap();
        assert_eq!(policy.backoff(1), Duration::from_millis(1));
        assert_eq!(policy.backoff(2), Duration::from_millis(2));
        assert_eq!(policy.backoff(8), Duration::from_millis(4));
        assert!(CeremonyWorkerHostPolicy::new(0, DurationMs::ZERO, DurationMs::ZERO,).is_err());
    }
}
