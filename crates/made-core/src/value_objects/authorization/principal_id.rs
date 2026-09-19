use serde::{Deserialize, Serialize};

use crate::DomainError;

const MAX_LEN: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PrincipalId(String);

impl PrincipalId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "principal_id",
            });
        }
        if trimmed.len() > MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "principal_id",
                actual: trimmed.len(),
                max: MAX_LEN,
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(DomainError::InvariantViolated {
                reason: "principal id cannot contain control characters",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for PrincipalId {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<PrincipalId> for String {
    fn from(value: PrincipalId) -> Self {
        value.0
    }
}
