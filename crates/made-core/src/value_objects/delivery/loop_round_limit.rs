use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

const MIN: u32 = 1;
const MAX: u32 = 10_000;
const DEFAULT_WITHOUT_PROGRESS: u32 = 3;

/// A count of rounds an integrator's loop is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct LoopRoundLimit(u32);

impl LoopRoundLimit {
    /// Construct a validated count.
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if !(MIN..=MAX).contains(&value) {
            return Err(DomainError::OutOfRange {
                field: "loop_round_limit",
                value: f64::from(value),
                min: f64::from(MIN),
                max: f64::from(MAX),
            });
        }
        Ok(Self(value))
    }

    /// How many rounds without progress are tolerated by default.
    #[must_use]
    pub const fn default_without_progress() -> Self {
        Self(DEFAULT_WITHOUT_PROGRESS)
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for LoopRoundLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for LoopRoundLimit {
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
    fn three_rounds_without_progress_is_the_default() {
        assert_eq!(LoopRoundLimit::default_without_progress().value(), 3);
        assert!(LoopRoundLimit::new(0).is_err());
    }
}
