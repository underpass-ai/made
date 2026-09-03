use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::SemanticSupportRule;

use super::output_contract_validation::{
    validate_text, MAX_ALLOWED_EVIDENCE_REFS, MAX_ALLOWED_VALUE_LEN, MAX_FIELD_NAME_LEN,
};

/// Field shape and evidence pack required to ground structured claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceGroundingRule {
    claims_field: String,
    refs_field: String,
    allowed_refs: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    semantic_support: Option<SemanticSupportRule>,
}

impl EvidenceGroundingRule {
    pub fn new(
        claims_field: impl Into<String>,
        refs_field: impl Into<String>,
        allowed_refs: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, DomainError> {
        let claims_field = validate_text(
            &claims_field.into(),
            "output_contract.evidence.claims_field",
            MAX_FIELD_NAME_LEN,
        )?;
        let refs_field = validate_text(
            &refs_field.into(),
            "output_contract.evidence.refs_field",
            MAX_FIELD_NAME_LEN,
        )?;
        let allowed_refs = allowed_refs
            .into_iter()
            .map(|reference| {
                validate_text(
                    &reference.into(),
                    "output_contract.evidence.allowed_ref",
                    MAX_ALLOWED_VALUE_LEN,
                )
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if allowed_refs.is_empty() {
            return Err(DomainError::EmptyField {
                field: "output_contract.evidence.allowed_refs",
            });
        }
        if allowed_refs.len() > MAX_ALLOWED_EVIDENCE_REFS {
            return Err(DomainError::OutOfRange {
                field: "output_contract.evidence.allowed_refs",
                value: allowed_refs.len() as f64,
                min: 1.0,
                max: MAX_ALLOWED_EVIDENCE_REFS as f64,
            });
        }
        Ok(Self {
            claims_field,
            refs_field,
            allowed_refs,
            semantic_support: None,
        })
    }

    pub fn with_semantic_support(mut self, rule: SemanticSupportRule) -> Result<Self, DomainError> {
        if self
            .allowed_refs
            .iter()
            .any(|reference| !rule.bodies.contains_key(reference))
        {
            return Err(DomainError::EmptyField {
                field: "output_contract.evidence.semantic_support.bodies",
            });
        }
        self.semantic_support = Some(rule);
        Ok(self)
    }

    #[must_use]
    pub fn claims_field(&self) -> &str {
        &self.claims_field
    }

    #[must_use]
    pub fn refs_field(&self) -> &str {
        &self.refs_field
    }

    #[must_use]
    pub fn allowed_refs(&self) -> &BTreeSet<String> {
        &self.allowed_refs
    }

    #[must_use]
    pub fn semantic_support(&self) -> Option<&SemanticSupportRule> {
        self.semantic_support.as_ref()
    }
}
