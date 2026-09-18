use crate::{error::DomainError, value_objects::DurationMs};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyTimeout(DurationMs);
impl CeremonyTimeout {
    pub fn new(duration: DurationMs) -> Result<Self, DomainError> {
        if duration == DurationMs::ZERO {
            Err(DomainError::MustBeNonZero {
                field: "ceremony_timeout",
            })
        } else {
            Ok(Self(duration))
        }
    }
    #[must_use]
    pub fn duration(self) -> DurationMs {
        self.0
    }
}
