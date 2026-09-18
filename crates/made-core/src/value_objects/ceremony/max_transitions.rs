use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Maximum number of transitions one ceremony instance may apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct MaxTransitions(u32);

impl MaxTransitions {
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "max_transitions",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for MaxTransitions {
    type Error = DomainError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<MaxTransitions> for u32 {
    fn from(value: MaxTransitions) -> Self {
        value.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_caps_are_positive() {
        assert_eq!(MaxTransitions::new(7).unwrap().get(), 7);
        assert!(matches!(
            MaxTransitions::new(0),
            Err(DomainError::MustBeNonZero {
                field: "max_transitions"
            })
        ));
    }
}
