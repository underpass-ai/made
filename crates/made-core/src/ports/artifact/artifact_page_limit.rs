use crate::DomainError;

use super::ARTIFACT_MAX_PAGE_ITEMS;

/// Validated metadata page size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactPageLimit(u16);

impl ArtifactPageLimit {
    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "artifact_page_limit",
            });
        }
        if value > ARTIFACT_MAX_PAGE_ITEMS {
            return Err(DomainError::OutOfRange {
                field: "artifact_page_limit",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(ARTIFACT_MAX_PAGE_ITEMS),
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl Default for ArtifactPageLimit {
    fn default() -> Self {
        Self(50)
    }
}
