use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Validated size of one council-journal page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "usize", into = "usize")]
pub struct CouncilJournalPageLimit(usize);

impl CouncilJournalPageLimit {
    pub const DEFAULT: Self = Self(200);
    pub const MAX: usize = 1000;

    pub fn new(value: usize) -> Result<Self, DomainError> {
        if value == 0 || value > Self::MAX {
            #[allow(clippy::cast_precision_loss)]
            return Err(DomainError::OutOfRange {
                field: "council_journal_page_limit",
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

impl Default for CouncilJournalPageLimit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<usize> for CouncilJournalPageLimit {
    type Error = DomainError;
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<CouncilJournalPageLimit> for usize {
    fn from(value: CouncilJournalPageLimit) -> Self {
        value.0
    }
}
