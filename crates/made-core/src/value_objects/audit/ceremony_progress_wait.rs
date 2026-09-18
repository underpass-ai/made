use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// How long a progress request waits for another sealed event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyProgressWait(u32);

impl CeremonyProgressWait {
    pub const DEFAULT: Self = Self(1_000);
    pub const MAX_MILLIS: u32 = 30_000;
    pub const IMMEDIATE: Self = Self(0);

    pub fn from_millis(value: u32) -> Result<Self, DomainError> {
        if value > Self::MAX_MILLIS {
            return Err(DomainError::OutOfRange {
                field: "ceremony_progress_wait_ms",
                value: f64::from(value),
                min: 0.0,
                max: f64::from(Self::MAX_MILLIS),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn millis(self) -> u32 {
        self.0
    }
}

impl Default for CeremonyProgressWait {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immediate_through_thirty_seconds_are_valid() {
        assert_eq!(CeremonyProgressWait::IMMEDIATE.millis(), 0);
        assert_eq!(
            CeremonyProgressWait::from_millis(30_000).unwrap().millis(),
            30_000
        );
        assert!(CeremonyProgressWait::from_millis(30_001).is_err());
    }
}
