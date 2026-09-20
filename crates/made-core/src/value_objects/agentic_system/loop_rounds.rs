use std::fmt;
use std::num::NonZeroU32;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

/// No design needs more rounds than this, and one that asks for them
/// has a runaway loop rather than a thorough one.
const MAX_ROUNDS: u32 = 100;

/// How many times a looping composition may run before the loop ends.
///
/// Bounded by construction: an unbounded loop in a design is a system
/// that can never be said to have finished, and the analysis refuses a
/// cycle that has no bound rather than letting a run discover it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct LoopRounds(NonZeroU32);

impl LoopRounds {
    pub fn new(value: u32) -> Result<Self, DomainError> {
        let value = NonZeroU32::new(value).ok_or(DomainError::MustBeNonZero {
            field: "loop_rounds",
        })?;
        if value.get() > MAX_ROUNDS {
            return Err(DomainError::OutOfRange {
                field: "loop_rounds",
                value: f64::from(value.get()),
                min: 1.0,
                max: f64::from(MAX_ROUNDS),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl fmt::Display for LoopRounds {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for LoopRounds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(u32::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loop_bound_is_positive_and_finite() {
        assert_eq!(LoopRounds::new(2).unwrap().get(), 2);
        assert!(LoopRounds::new(0).is_err());
        assert!(LoopRounds::new(MAX_ROUNDS + 1).is_err());
    }

    #[test]
    fn a_stored_zero_is_refused_on_the_way_in() {
        assert!(serde_json::from_str::<LoopRounds>("0").is_err());
        assert_eq!(serde_json::from_str::<LoopRounds>("3").unwrap().get(), 3);
    }
}
