use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Operator-visible reason a consumer stopped retrying one event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyEventQuarantineReason(String);

impl CeremonyEventQuarantineReason {
    pub const MAX_LEN: usize = 512;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "ceremony_event_quarantine_reason",
            });
        }
        if value.len() > Self::MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "ceremony_event_quarantine_reason",
                actual: value.len(),
                max: Self::MAX_LEN,
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
