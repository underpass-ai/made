use serde::{Deserialize, Serialize};

use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AuthorizationGrantId(String);
impl AuthorizationGrantId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.len() > 256 || trimmed.chars().any(char::is_control) {
            return Err(DomainError::InvariantViolated {
                reason: "authorization grant id must be non-empty, bounded and printable",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for AuthorizationGrantId {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<AuthorizationGrantId> for String {
    fn from(value: AuthorizationGrantId) -> Self {
        value.0
    }
}
