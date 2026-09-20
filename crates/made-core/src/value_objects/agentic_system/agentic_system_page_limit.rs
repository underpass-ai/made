use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

/// The most designs one listing may return.
const MAX_LIMIT: u16 = 100;
const DEFAULT_LIMIT: u16 = 50;

/// How many designs one page of a catalogue holds.
///
/// Bounded at the type rather than at each adapter, so a listing
/// cannot be made unbounded by a caller sending a large number and an
/// adapter forgetting to clamp it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct AgenticSystemPageLimit(u16);

impl AgenticSystemPageLimit {
    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "agentic_system_page_limit",
            });
        }
        if value > MAX_LIMIT {
            return Err(DomainError::OutOfRange {
                field: "agentic_system_page_limit",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(MAX_LIMIT),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl Default for AgenticSystemPageLimit {
    fn default() -> Self {
        Self(DEFAULT_LIMIT)
    }
}

impl fmt::Display for AgenticSystemPageLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for AgenticSystemPageLimit {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(u16::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_at_least_one_and_never_unbounded() {
        assert_eq!(AgenticSystemPageLimit::default().get(), DEFAULT_LIMIT);
        assert!(AgenticSystemPageLimit::new(0).is_err());
        assert!(AgenticSystemPageLimit::new(MAX_LIMIT + 1).is_err());
        assert_eq!(AgenticSystemPageLimit::new(1).unwrap().as_usize(), 1);
    }
}
