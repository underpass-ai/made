use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Confidence percentage attached to an evidence-support decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SupportConfidence(u8);

impl SupportConfidence {
    pub fn new(percent: u8) -> Result<Self, DomainError> {
        if percent > 100 {
            return Err(DomainError::OutOfRange {
                field: "evidence_support.confidence",
                value: f64::from(percent),
                min: 0.0,
                max: 100.0,
            });
        }
        Ok(Self(percent))
    }

    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn meets(self, minimum_percent: u8) -> bool {
        self.0 >= minimum_percent
    }
}
