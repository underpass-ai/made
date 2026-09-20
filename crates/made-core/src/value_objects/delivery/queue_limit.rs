use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

const MIN: u32 = 1;
const MAX: u32 = 1_000;
const DEFAULT: u32 = 200;

/// How many undelivered items one destination may hold before the
/// oldest non-blocking one is dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct QueueLimit(u32);

impl QueueLimit {
    /// Construct a validated limit.
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if !(MIN..=MAX).contains(&value) {
            return Err(DomainError::OutOfRange {
                field: "queue_limit",
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
}

impl Default for QueueLimit {
    fn default() -> Self {
        Self(DEFAULT)
    }
}

impl fmt::Display for QueueLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for QueueLimit {
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
    fn the_default_is_two_hundred_and_the_ceiling_is_a_thousand() {
        assert_eq!(QueueLimit::default().value(), 200);
        assert!(QueueLimit::new(0).is_err());
        assert!(QueueLimit::new(1_001).is_err());
    }
}
