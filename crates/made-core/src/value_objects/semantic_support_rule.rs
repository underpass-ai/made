use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::output_contract_validation::{
    validate_text, MAX_ALLOWED_EVIDENCE_REFS, MAX_ALLOWED_VALUE_LEN, MAX_EVIDENCE_BODY_LEN,
};

/// Default minimum confidence required for semantic evidence support.
pub const DEFAULT_SUPPORT_MIN_CONFIDENCE: u8 = 70;

/// Evidence bodies and acceptance threshold for semantic support judging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticSupportRule {
    min_confidence: u8,
    pub(super) bodies: BTreeMap<String, String>,
}

impl SemanticSupportRule {
    pub fn new(
        min_confidence: u8,
        bodies: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Result<Self, DomainError> {
        if min_confidence > 100 {
            return Err(DomainError::OutOfRange {
                field: "output_contract.evidence.semantic_support.min_confidence",
                value: f64::from(min_confidence),
                min: 0.0,
                max: 100.0,
            });
        }
        let bodies = bodies
            .into_iter()
            .map(|(reference, body)| {
                let reference = validate_text(
                    &reference.into(),
                    "output_contract.evidence.semantic_support.body_ref",
                    MAX_ALLOWED_VALUE_LEN,
                )?;
                let body = validate_text(
                    &body.into(),
                    "output_contract.evidence.semantic_support.body",
                    MAX_EVIDENCE_BODY_LEN,
                )?;
                Ok::<_, DomainError>((reference, body))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        if bodies.is_empty() {
            return Err(DomainError::EmptyField {
                field: "output_contract.evidence.semantic_support.bodies",
            });
        }
        if bodies.len() > MAX_ALLOWED_EVIDENCE_REFS {
            return Err(DomainError::OutOfRange {
                field: "output_contract.evidence.semantic_support.bodies",
                value: bodies.len() as f64,
                min: 1.0,
                max: MAX_ALLOWED_EVIDENCE_REFS as f64,
            });
        }
        Ok(Self {
            min_confidence,
            bodies,
        })
    }

    #[must_use]
    pub const fn min_confidence(&self) -> u8 {
        self.min_confidence
    }

    #[must_use]
    pub fn bodies(&self) -> &BTreeMap<String, String> {
        &self.bodies
    }

    #[must_use]
    pub fn body(&self, reference: &str) -> Option<&str> {
        self.bodies.get(reference).map(String::as_str)
    }
}
