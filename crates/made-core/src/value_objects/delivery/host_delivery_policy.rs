use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::DurationMs;

use super::{DeliveryAttemptLimit, FollowReplacement, HostDeliveryMode};

const MIN_LEASE_MS: u64 = 1;
const MAX_LEASE_MS: u64 = 3_600_000;
const DEFAULT_LEASE_MS: u64 = 60_000;

/// The terms one delivery is offered under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostDeliveryPolicy {
    mode: HostDeliveryMode,
    lease_duration: DurationMs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ack_timeout: Option<DurationMs>,
    max_attempts: DeliveryAttemptLimit,
    follow_replacement: FollowReplacement,
}

impl HostDeliveryPolicy {
    /// Construct a validated policy.
    ///
    /// The lease is bounded at both ends on purpose. A zero-length lease
    /// is exclusive to nobody, and an unbounded one strands an item
    /// behind a host that never comes back.
    pub fn new(
        mode: HostDeliveryMode,
        lease_duration: DurationMs,
        ack_timeout: Option<DurationMs>,
        max_attempts: DeliveryAttemptLimit,
        follow_replacement: FollowReplacement,
    ) -> Result<Self, DomainError> {
        if !(MIN_LEASE_MS..=MAX_LEASE_MS).contains(&lease_duration.get()) {
            return Err(DomainError::OutOfRange {
                field: "host_delivery_lease_duration",
                value: lease_duration.get() as f64,
                min: MIN_LEASE_MS as f64,
                max: MAX_LEASE_MS as f64,
            });
        }
        if ack_timeout.is_some_and(|timeout| timeout.get() == 0) {
            return Err(DomainError::MustBeNonZero {
                field: "host_delivery_ack_timeout",
            });
        }
        Ok(Self {
            mode,
            lease_duration,
            ack_timeout,
            max_attempts,
            follow_replacement,
        })
    }

    /// The defaults for a destination that pulls its own work.
    #[must_use]
    pub fn pull() -> Self {
        Self {
            mode: HostDeliveryMode::PullLease,
            lease_duration: DurationMs::from_millis(DEFAULT_LEASE_MS),
            ack_timeout: None,
            max_attempts: DeliveryAttemptLimit::default(),
            follow_replacement: FollowReplacement::default(),
        }
    }

    /// The defaults for a destination the engine wakes.
    #[must_use]
    pub fn activation() -> Self {
        Self {
            mode: HostDeliveryMode::Activation,
            ..Self::pull()
        }
    }

    #[must_use]
    pub const fn mode(&self) -> HostDeliveryMode {
        self.mode
    }

    #[must_use]
    pub const fn lease_duration(&self) -> DurationMs {
        self.lease_duration
    }

    /// How long an acknowledged delivery may sit before it expires.
    #[must_use]
    pub const fn ack_timeout(&self) -> Option<DurationMs> {
        self.ack_timeout
    }

    #[must_use]
    pub const fn max_attempts(&self) -> DeliveryAttemptLimit {
        self.max_attempts
    }

    #[must_use]
    pub const fn follow_replacement(&self) -> FollowReplacement {
        self.follow_replacement
    }

    /// The same terms, with replacement following turned on.
    #[must_use]
    pub fn following_replacement(mut self) -> Self {
        self.follow_replacement = FollowReplacement::Follow;
        self
    }
}

impl Default for HostDeliveryPolicy {
    fn default() -> Self {
        Self::pull()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lease_of_no_length_and_one_of_a_day_are_both_refused() {
        let refused = |millis| {
            HostDeliveryPolicy::new(
                HostDeliveryMode::PullLease,
                DurationMs::from_millis(millis),
                None,
                DeliveryAttemptLimit::default(),
                FollowReplacement::Stay,
            )
        };
        assert!(refused(0).is_err());
        assert!(refused(86_400_000).is_err());
        assert!(refused(1).is_ok());
    }

    #[test]
    fn the_defaults_pull_for_a_minute_and_retry_three_times() {
        let policy = HostDeliveryPolicy::default();
        assert_eq!(policy.mode(), HostDeliveryMode::PullLease);
        assert_eq!(policy.lease_duration().get(), 60_000);
        assert_eq!(policy.max_attempts().value(), 3);
        assert!(!policy.follow_replacement().follows());
        assert!(policy
            .following_replacement()
            .follow_replacement()
            .follows());
    }
}
