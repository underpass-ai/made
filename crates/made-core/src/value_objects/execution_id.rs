use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_EXECUTION_ID_LEN: usize = 128;

/// Stable identity assigned by an execution adapter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExecutionId(String);

impl ExecutionId {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "execution.id",
            });
        }
        if value.len() > MAX_EXECUTION_ID_LEN {
            return Err(DomainError::FieldTooLong {
                field: "execution.id",
                actual: value.len(),
                max: MAX_EXECUTION_ID_LEN,
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExecutionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_keeps_a_valid_identity() {
        assert_eq!(
            ExecutionId::new(" invocation-1 ").unwrap().as_str(),
            "invocation-1"
        );
    }

    #[test]
    fn rejects_an_empty_identity() {
        assert!(matches!(
            ExecutionId::new("  ").unwrap_err(),
            DomainError::EmptyField {
                field: "execution.id"
            }
        ));
    }
}
