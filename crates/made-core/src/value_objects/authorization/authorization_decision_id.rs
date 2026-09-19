use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AuthorizationDecisionId(String);

impl AuthorizationDecisionId {
    #[must_use]
    pub fn for_bytes(value: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(value)))
    }

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DomainError::InvariantViolated {
                reason: "authorization decision id must be a lowercase sha256 value",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AuthorizationDecisionId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AuthorizationDecisionId> for String {
    fn from(value: AuthorizationDecisionId) -> Self {
        value.0
    }
}
