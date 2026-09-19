use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Unique token proving ownership of one cursor lease.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CouncilJournalLeaseId(String);

impl CouncilJournalLeaseId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "council_journal_lease_id",
            });
        }
        if value.len() > 128 {
            return Err(DomainError::FieldTooLong {
                field: "council_journal_lease_id",
                actual: value.len(),
                max: 128,
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CouncilJournalLeaseId {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<CouncilJournalLeaseId> for String {
    fn from(value: CouncilJournalLeaseId) -> Self {
        value.0
    }
}
