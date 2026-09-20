use crate::error::DomainError;
use serde::{Deserialize, Serialize};
use std::fmt;
const MAX_LEN: usize = 256;
/// Identity of a ceremony participant independent of the host executor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LogicalWorkerId(String);
impl LogicalWorkerId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "logical_worker_id",
            });
        }
        if value.len() > MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "logical_worker_id",
                actual: value.len(),
                max: MAX_LEN,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "logical_worker_id",
            });
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for LogicalWorkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
