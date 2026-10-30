use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// A durable entry into a state, independent of within-state repetition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct StateVisit(u32);

impl StateVisit {
    pub const FIRST: Self = Self(1);

    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "state_visit",
            });
        }
        Ok(Self(value))
    }

    pub fn next(self) -> Result<Self, DomainError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(DomainError::InvariantViolated {
                reason: "state visit coordinate exhausted",
            })
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn is_first(&self) -> bool {
        self.0 == 1
    }
}

impl Default for StateVisit {
    fn default() -> Self {
        Self::FIRST
    }
}
impl TryFrom<u32> for StateVisit {
    type Error = DomainError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<StateVisit> for u32 {
    fn from(value: StateVisit) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visits_refuse_zero_and_overflow() {
        assert!(StateVisit::new(0).is_err());
        assert!(serde_json::from_str::<StateVisit>("0").is_err());
        assert!(StateVisit::new(u32::MAX).unwrap().next().is_err());
        assert_eq!(StateVisit::FIRST.next().unwrap().get(), 2);
        assert_eq!(serde_json::to_string(&StateVisit::FIRST).unwrap(), "1");
    }
}
