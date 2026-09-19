use serde::{Deserialize, Serialize};

use crate::DomainError;

const MAX: u8 = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DelegationDepth(u8);
impl DelegationDepth {
    #[must_use]
    pub const fn none() -> Self {
        Self(0)
    }
    pub fn new(value: u8) -> Result<Self, DomainError> {
        if value > MAX {
            return Err(DomainError::OutOfRange {
                field: "delegation_depth",
                value: f64::from(value),
                min: 0.0,
                max: f64::from(MAX),
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
    pub fn delegated(self) -> Result<Self, DomainError> {
        self.0
            .checked_sub(1)
            .map(Self)
            .ok_or(DomainError::InvariantViolated {
                reason: "authorization grant is not delegable",
            })
    }
}
