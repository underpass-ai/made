use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepAttempt(u32);

impl StepAttempt {
    pub const FIRST: Self = Self(1);

    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "step_attempt",
            });
        }
        Ok(Self(value))
    }

    pub fn next(self) -> Result<Self, DomainError> {
        Self::new(self.0.saturating_add(1))
    }

    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }
}
