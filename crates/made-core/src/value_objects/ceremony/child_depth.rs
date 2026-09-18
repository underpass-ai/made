use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct ChildDepth(u16);

impl ChildDepth {
    pub const FIRST: Self = Self(1);
    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "child_depth",
            });
        }
        Ok(Self(value))
    }
    pub fn next(self) -> Result<Self, DomainError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(DomainError::InvariantViolated {
                reason: "child ceremony depth exhausted",
            })
    }
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}
impl TryFrom<u16> for ChildDepth {
    type Error = DomainError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<ChildDepth> for u16 {
    fn from(value: ChildDepth) -> Self {
        value.0
    }
}
