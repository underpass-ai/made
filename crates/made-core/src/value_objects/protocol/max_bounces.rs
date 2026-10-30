use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Maximum number of times one exact declared edge may be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct MaxBounces(u32);

impl MaxBounces {
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "max_bounces",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for MaxBounces {
    type Error = DomainError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<MaxBounces> for u32 {
    fn from(value: MaxBounces) -> Self {
        value.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_caps_are_positive() {
        assert_eq!(MaxBounces::new(3).unwrap().get(), 3);
        assert!(matches!(
            MaxBounces::new(0),
            Err(DomainError::MustBeNonZero {
                field: "max_bounces"
            })
        ));
    }
}
