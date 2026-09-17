use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Stable name of one independent consumer of the global ceremony-event feed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyEventConsumer(String);

impl CeremonyEventConsumer {
    pub const MAX_LEN: usize = 128;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "ceremony_event_consumer",
            });
        }
        if value.len() > Self::MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "ceremony_event_consumer",
                actual: value.len(),
                max: Self::MAX_LEN,
            });
        }
        if !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
        {
            return Err(DomainError::InvalidCharacters {
                field: "ceremony_event_consumer",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
