use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

use super::DeliveryAttempt;

const MIN: u32 = 1;
const MAX: u32 = 100;
const DEFAULT: u32 = 3;

/// How many failed attempts a delivery is worth before it stays failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct DeliveryAttemptLimit(u32);

impl DeliveryAttemptLimit {
    /// Construct a validated limit.
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if !(MIN..=MAX).contains(&value) {
            return Err(DomainError::OutOfRange {
                field: "delivery_attempt_limit",
                value: f64::from(value),
                min: f64::from(MIN),
                max: f64::from(MAX),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Whether this many attempts exhausts the delivery.
    #[must_use]
    pub const fn is_exhausted_by(self, attempt: DeliveryAttempt) -> bool {
        attempt.value() >= self.0
    }
}

impl Default for DeliveryAttemptLimit {
    fn default() -> Self {
        Self(DEFAULT)
    }
}

impl fmt::Display for DeliveryAttemptLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for DeliveryAttemptLimit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u32::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_three_and_the_range_is_closed() {
        assert_eq!(DeliveryAttemptLimit::default().value(), 3);
        assert!(DeliveryAttemptLimit::new(0).is_err());
        assert!(DeliveryAttemptLimit::new(101).is_err());
        assert!(serde_json::from_str::<DeliveryAttemptLimit>("0").is_err());
    }

    #[test]
    fn exhaustion_is_reached_at_the_limit() {
        let limit = DeliveryAttemptLimit::new(2).unwrap();
        assert!(!limit.is_exhausted_by(DeliveryAttempt::NONE.next()));
        assert!(limit.is_exhausted_by(DeliveryAttempt::NONE.next().next()));
    }
}
