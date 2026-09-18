use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct MaxChildren(u16);

impl MaxChildren {
    pub const SERVER_MAX: Self = Self(64);

    pub fn new(value: u16) -> Result<Self, DomainError> {
        if !(1..=Self::SERVER_MAX.0).contains(&value) {
            return Err(DomainError::OutOfRange {
                field: "max_children",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(Self::SERVER_MAX.0),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for MaxChildren {
    type Error = DomainError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<MaxChildren> for u16 {
    fn from(value: MaxChildren) -> Self {
        value.0
    }
}
