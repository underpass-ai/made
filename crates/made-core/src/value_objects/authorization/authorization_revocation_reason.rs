use serde::{Deserialize, Serialize};

use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AuthorizationRevocationReason(String);
impl AuthorizationRevocationReason {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.len() > 512 || trimmed.chars().any(char::is_control) {
            return Err(DomainError::InvariantViolated {
                reason: "authorization revocation reason must be non-empty, bounded and printable",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for AuthorizationRevocationReason {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<AuthorizationRevocationReason> for String {
    fn from(value: AuthorizationRevocationReason) -> Self {
        value.0
    }
}
