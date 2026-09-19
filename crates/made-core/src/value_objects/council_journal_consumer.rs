use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Stable name of one independent consumer of the council journal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CouncilJournalConsumer(String);

impl CouncilJournalConsumer {
    pub const MAX_LEN: usize = 128;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "council_journal_consumer",
            });
        }
        if value.len() > Self::MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "council_journal_consumer",
                actual: value.len(),
                max: Self::MAX_LEN,
            });
        }
        if !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
        {
            return Err(DomainError::InvalidCharacters {
                field: "council_journal_consumer",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CouncilJournalConsumer {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<CouncilJournalConsumer> for String {
    fn from(value: CouncilJournalConsumer) -> Self {
        value.0
    }
}
