use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_LENGTH: usize = 512;

/// Opaque operation identity observed from an external connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExternalOperationId(String);

impl ExternalOperationId {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "external_operation_id",
            });
        }
        if value.len() > MAX_LENGTH {
            return Err(DomainError::FieldTooLong {
                field: "external_operation_id",
                actual: value.len(),
                max: MAX_LENGTH,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "external_operation_id",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ExternalOperationId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ExternalOperationId> for String {
    fn from(value: ExternalOperationId) -> Self {
        value.0
    }
}
