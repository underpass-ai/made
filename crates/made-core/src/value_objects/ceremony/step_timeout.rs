use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::DurationMs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepTimeout(DurationMs);

impl StepTimeout {
    pub fn new(duration: DurationMs) -> Result<Self, DomainError> {
        if duration == DurationMs::ZERO {
            return Err(DomainError::MustBeNonZero {
                field: "step_timeout",
            });
        }
        Ok(Self(duration))
    }

    #[must_use]
    pub fn duration(self) -> DurationMs {
        self.0
    }
}
