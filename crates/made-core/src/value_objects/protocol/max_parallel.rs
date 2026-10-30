use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct MaxParallel(u8);

impl MaxParallel {
    pub const DEFAULT: Self = Self(3);
    pub const SERVER_MAX: Self = Self(8);

    pub fn new(value: u8) -> Result<Self, DomainError> {
        if !(1..=8).contains(&value) {
            return Err(DomainError::OutOfRange {
                field: "max_parallel",
                value: f64::from(value),
                min: 1.0,
                max: 8.0,
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn effective_with(self, ceiling: Self) -> Self {
        if self.0 < ceiling.0 {
            self
        } else {
            ceiling
        }
    }

    #[must_use]
    pub const fn is_default(&self) -> bool {
        self.0 == Self::DEFAULT.0
    }
}

impl Default for MaxParallel {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u8> for MaxParallel {
    type Error = DomainError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<MaxParallel> for u8 {
    fn from(value: MaxParallel) -> Self {
        value.get()
    }
}
