use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Maximum number of protocol instances returned by one public page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolInstancePageLimit(u16);

impl ProtocolInstancePageLimit {
    pub const DEFAULT: Self = Self(50);
    pub const MAX: u16 = 100;

    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value == 0 || value > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "protocol_instance_page_limit",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(Self::MAX),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> usize {
        self.0 as usize
    }
}

impl Default for ProtocolInstancePageLimit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_limit_is_nonzero_and_bounded() {
        assert_eq!(ProtocolInstancePageLimit::DEFAULT.value(), 50);
        assert_eq!(ProtocolInstancePageLimit::new(100).unwrap().value(), 100);
        assert!(ProtocolInstancePageLimit::new(0).is_err());
        assert!(ProtocolInstancePageLimit::new(101).is_err());
    }
}
