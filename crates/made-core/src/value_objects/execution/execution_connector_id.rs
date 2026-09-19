use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_LENGTH: usize = 128;

/// Stable configured identity of an execution connector.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExecutionConnectorId(String);

impl ExecutionConnectorId {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "execution_connector_id",
            });
        }
        if value.len() > MAX_LENGTH {
            return Err(DomainError::FieldTooLong {
                field: "execution_connector_id",
                actual: value.len(),
                max: MAX_LENGTH,
            });
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
        {
            return Err(DomainError::InvalidCharacters {
                field: "execution_connector_id",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ExecutionConnectorId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ExecutionConnectorId> for String {
    fn from(value: ExecutionConnectorId) -> Self {
        value.0
    }
}

impl fmt::Display for ExecutionConnectorId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
