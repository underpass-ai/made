use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct JoinStepCount(u32);

impl JoinStepCount {
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "join_step_count",
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for JoinStepCount {
    type Error = DomainError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<JoinStepCount> for u32 {
    fn from(value: JoinStepCount) -> Self {
        value.get()
    }
}
