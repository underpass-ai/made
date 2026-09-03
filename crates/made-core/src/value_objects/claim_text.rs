use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Claim text submitted to the evidence-support decision port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClaimText(String);

impl ClaimText {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "evidence_support.claim",
            });
        }
        Ok(Self(raw))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
