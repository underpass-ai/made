use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Bounded number of execution operations returned by one recovery scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct ExecutionRecoveryPageLimit(u16);

impl ExecutionRecoveryPageLimit {
    pub const DEFAULT: Self = Self(100);
    pub const MAX: u16 = 1000;

    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "execution_recovery_page_limit",
            });
        }
        if value > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "execution_recovery_page_limit",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(Self::MAX),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for ExecutionRecoveryPageLimit {
    type Error = DomainError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ExecutionRecoveryPageLimit> for u16 {
    fn from(value: ExecutionRecoveryPageLimit) -> Self {
        value.0
    }
}
