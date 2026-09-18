use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::MaxChildDepth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct ChildDepthBudget(u16);

impl ChildDepthBudget {
    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value > MaxChildDepth::SERVER_MAX.get() {
            return Err(DomainError::OutOfRange {
                field: "child_depth_budget",
                value: f64::from(value),
                min: 0.0,
                max: f64::from(MaxChildDepth::SERVER_MAX.get()),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn permits_child(self) -> bool {
        self.0 > 0
    }

    pub fn for_child(self, local: MaxChildDepth) -> Result<Self, DomainError> {
        let effective = self.0.min(local.get());
        if effective == 0 {
            return Err(DomainError::InvariantViolated {
                reason: "child ceremony depth budget is exhausted",
            });
        }
        Self::new(effective - 1)
    }
}

impl From<MaxChildDepth> for ChildDepthBudget {
    fn from(value: MaxChildDepth) -> Self {
        Self(value.get())
    }
}
impl TryFrom<u16> for ChildDepthBudget {
    type Error = DomainError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<ChildDepthBudget> for u16 {
    fn from(value: ChildDepthBudget) -> Self {
        value.0
    }
}
