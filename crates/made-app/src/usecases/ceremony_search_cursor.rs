use serde::{Deserialize, Serialize};

use made_core::error::DomainError;

const MAX_ENCODED_CURSOR_BYTES: usize = 1024;

/// Opaque continuation token returned by the public ceremony search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonySearchCursor(String);

impl CeremonySearchCursor {
    pub(super) fn new(encoded: String) -> Self {
        Self(encoded)
    }

    pub fn parse(encoded: impl Into<String>) -> Result<Self, DomainError> {
        let encoded = encoded.into();
        if encoded.is_empty() {
            return Err(DomainError::EmptyField {
                field: "ceremony_search_cursor",
            });
        }
        if encoded.len() > MAX_ENCODED_CURSOR_BYTES {
            return Err(DomainError::FieldTooLong {
                field: "ceremony_search_cursor",
                actual: encoded.len(),
                max: MAX_ENCODED_CURSOR_BYTES,
            });
        }
        if !encoded
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(DomainError::InvalidCharacters {
                field: "ceremony_search_cursor",
            });
        }
        Ok(Self(encoded))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
