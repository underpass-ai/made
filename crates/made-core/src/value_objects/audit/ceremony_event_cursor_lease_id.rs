use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Unique token proving ownership of one cursor lease.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyEventCursorLeaseId(String);

impl CeremonyEventCursorLeaseId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "ceremony_event_cursor_lease_id",
            });
        }
        if value.len() > 128 {
            return Err(DomainError::FieldTooLong {
                field: "ceremony_event_cursor_lease_id",
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
