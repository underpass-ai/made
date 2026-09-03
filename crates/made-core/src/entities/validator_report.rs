use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::Attributes;

/// A single adapter-defined validator result for one proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorReport {
    kind: String,
    passed: bool,
    summary: String,
    details: Attributes,
}

impl ValidatorReport {
    pub fn new(
        kind: impl Into<String>,
        passed: bool,
        summary: impl Into<String>,
        details: Attributes,
    ) -> Result<Self, DomainError> {
        let kind = kind.into();
        if kind.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "validator_report.kind",
            });
        }
        Ok(Self {
            kind,
            passed,
            summary: summary.into(),
            details,
        })
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn passed(&self) -> bool {
        self.passed
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn details(&self) -> &Attributes {
        &self.details
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_kind_is_rejected() {
        let error = ValidatorReport::new("  ", true, "", Attributes::empty()).unwrap_err();
        assert!(matches!(
            error,
            DomainError::EmptyField {
                field: "validator_report.kind"
            }
        ));
    }

    #[test]
    fn arbitrary_kind_is_accepted() {
        for kind in [
            "lint",
            "policy",
            "dry-run",
            "clinical-safety",
            "sourcing-feasibility",
            "fact-check",
        ] {
            let report = ValidatorReport::new(kind, true, "summary", Attributes::empty()).unwrap();
            assert_eq!(report.kind(), kind);
        }
    }
}
