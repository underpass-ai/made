use crate::error::DomainError;
use serde::{Deserialize, Serialize};

/// An ordinal in the council journal. It cannot be a ceremony cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct CouncilJournalPosition(u64);

impl CouncilJournalPosition {
    pub const FIRST: Self = Self(1);
    pub fn new(value: u64) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "council_journal_position",
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }
    pub fn checked_next(self) -> Result<Self, DomainError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(DomainError::InvariantViolated {
                reason: "council journal position exhausted",
            })
    }
}
impl TryFrom<u64> for CouncilJournalPosition {
    type Error = DomainError;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<CouncilJournalPosition> for u64 {
    fn from(value: CouncilJournalPosition) -> Self {
        value.0
    }
}
