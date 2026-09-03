use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Identity of one evidence item cited by a claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceReference(String);

impl EvidenceReference {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "evidence_support.reference",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
