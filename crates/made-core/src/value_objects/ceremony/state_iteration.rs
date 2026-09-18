use serde::{Deserialize, Serialize};

use crate::error::DomainError;

pub const MAX_STATE_ITERATIONS: u32 = 1_000;

/// One complete execution of every step in a ceremony state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StateIteration(u32);

impl StateIteration {
    pub const FIRST: Self = Self(1);

    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "state_iteration",
            });
        }
        if value > MAX_STATE_ITERATIONS {
            return Err(DomainError::OutOfRange {
                field: "state_iteration",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(MAX_STATE_ITERATIONS),
            });
        }
        Ok(Self(value))
    }

    pub fn next(self) -> Result<Self, DomainError> {
        Self::new(self.0.saturating_add(1))
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

impl Default for StateIteration {
    fn default() -> Self {
        Self::FIRST
    }
}
