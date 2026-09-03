use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_COUNCIL_CONTRACT_ID_LEN: usize = 128;

/// Stable identity of the council contract attached to a task.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CouncilContractId(String);

impl CouncilContractId {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "task_metadata.council_contract_id",
            });
        }
        if value.len() > MAX_COUNCIL_CONTRACT_ID_LEN {
            return Err(DomainError::FieldTooLong {
                field: "task_metadata.council_contract_id",
                actual: value.len(),
                max: MAX_COUNCIL_CONTRACT_ID_LEN,
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CouncilContractId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<str> for CouncilContractId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for CouncilContractId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_valid_identity() {
        assert_eq!(
            CouncilContractId::new(" council-v1 ").unwrap(),
            "council-v1"
        );
    }

    #[test]
    fn rejects_empty_identity() {
        assert!(CouncilContractId::new(" ").is_err());
    }
}
