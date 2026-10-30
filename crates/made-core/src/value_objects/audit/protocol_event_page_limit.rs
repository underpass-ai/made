use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Validated size of one protocol-event page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolEventPageLimit(usize);

impl ProtocolEventPageLimit {
    pub const DEFAULT: Self = Self(200);
    pub const MAX: usize = 1000;

    pub fn new(value: usize) -> Result<Self, DomainError> {
        if value == 0 || value > Self::MAX {
            #[allow(clippy::cast_precision_loss)]
            return Err(DomainError::OutOfRange {
                field: "protocol_event_page_limit",
                value: value as f64,
                min: 1.0,
                max: Self::MAX as f64,
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

impl Default for ProtocolEventPageLimit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_bounded_pages_are_accepted() {
        assert_eq!(ProtocolEventPageLimit::new(1).unwrap().value(), 1);
        assert_eq!(
            ProtocolEventPageLimit::new(ProtocolEventPageLimit::MAX)
                .unwrap()
                .value(),
            ProtocolEventPageLimit::MAX
        );
        assert!(ProtocolEventPageLimit::new(0).is_err());
        assert!(ProtocolEventPageLimit::new(ProtocolEventPageLimit::MAX + 1).is_err());
    }
}
