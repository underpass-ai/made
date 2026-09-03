use std::borrow::Borrow;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_OUTPUT_CONTRACT_ID_LEN: usize = 128;

/// Stable identity of a structured output contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputContractId(String);

impl OutputContractId {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "output_contract.contract_id",
            });
        }
        if value.len() > MAX_OUTPUT_CONTRACT_ID_LEN {
            return Err(DomainError::FieldTooLong {
                field: "output_contract.contract_id",
                actual: value.len(),
                max: MAX_OUTPUT_CONTRACT_ID_LEN,
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for OutputContractId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for OutputContractId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<str> for OutputContractId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for OutputContractId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_a_valid_identity() {
        assert_eq!(OutputContractId::new(" report-v1 ").unwrap(), "report-v1");
    }

    #[test]
    fn rejects_an_empty_identity() {
        assert!(OutputContractId::new(" ").is_err());
    }
}
