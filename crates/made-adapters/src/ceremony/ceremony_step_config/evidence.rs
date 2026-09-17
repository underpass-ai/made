use made_core::error::DomainError;
use serde_json::Value;

use super::super::{EvidenceGroundingSpec, SemanticSupportSpec};
use super::{
    contract_key, evidence_key, field, key, optional_string, semantic_key, string_array,
    CeremonyStepConfig, DEFAULT_CLAIMS_FIELD, DEFAULT_REFS_FIELD,
};

impl CeremonyStepConfig<'_> {
    /// Evidence-grounding declaration inside the step's contract, if
    /// any.
    ///
    /// Shape (inside the `output_contract` block):
    ///
    /// ```yaml
    /// output_contract:
    ///   contract_id: evidence-bound-decision
    ///   evidence:
    ///     claims_field: claims                    # optional, default "claims"
    ///     refs_field: evidence_refs               # optional, default "evidence_refs"
    ///     allowed_refs: [ev-static-1]             # optional, static pack entries
    ///     allowed_refs_from_context: evidence_pack # optional, ceremony-context key
    ///     semantic_support:                        # optional, second gate
    ///       min_confidence: 70                     # optional, percent 0-100
    ///       bodies_from_context: evidence_pack     # optional, ceremony-context key
    /// ```
    ///
    /// At least one of `allowed_refs` / `allowed_refs_from_context` is
    /// required, unknown keys are rejected (same reasoning as the
    /// contract block: a typo must not silently weaken a policy gate).
    /// The context key is resolved by the transport layer at request
    /// time — the context entry must be an array of strings, or of
    /// objects each carrying a string `id` (the natural shape of an
    /// evidence pack).
    ///
    /// `semantic_support` (presence turns the gate on, `{}` is valid)
    /// additionally demands that every claim's cited evidence
    /// *supports* the claim, judged through the deployment's
    /// evidence-support judge. Its bodies resolve from
    /// `bodies_from_context` — defaulting to
    /// `allowed_refs_from_context` — whose entries must be objects
    /// carrying `id` and `text`.
    pub(crate) fn evidence_grounding_spec(
        &self,
    ) -> Result<Option<EvidenceGroundingSpec>, DomainError> {
        let Some(contract) = self.attributes.get(key::OUTPUT_CONTRACT) else {
            return Ok(None);
        };
        let Some(block) = contract.as_object() else {
            // output_contract() already rejects this shape; stay quiet here.
            return Ok(None);
        };
        let Some(value) = block.get(contract_key::EVIDENCE) else {
            return Ok(None);
        };
        if value.is_null() {
            return Ok(None);
        }
        let Some(evidence) = value.as_object() else {
            return Err(DomainError::InvalidCharacters {
                field: field::CONTRACT_EVIDENCE,
            });
        };

        let known = [
            evidence_key::CLAIMS_FIELD,
            evidence_key::REFS_FIELD,
            evidence_key::ALLOWED_REFS,
            evidence_key::ALLOWED_REFS_FROM_CONTEXT,
            evidence_key::SEMANTIC_SUPPORT,
        ];
        if evidence.keys().any(|key| !known.contains(&key.as_str())) {
            return Err(DomainError::InvalidCharacters {
                field: field::CONTRACT_EVIDENCE,
            });
        }

        if let Some(value) = evidence.get(evidence_key::CLAIMS_FIELD) {
            if !value.is_null() && !value.is_string() {
                return Err(DomainError::InvalidCharacters {
                    field: field::EVIDENCE_CLAIMS_FIELD,
                });
            }
        }
        if let Some(value) = evidence.get(evidence_key::REFS_FIELD) {
            if !value.is_null() && !value.is_string() {
                return Err(DomainError::InvalidCharacters {
                    field: field::EVIDENCE_REFS_FIELD,
                });
            }
        }
        let claims_field = optional_string(evidence.get(evidence_key::CLAIMS_FIELD))
            .unwrap_or(DEFAULT_CLAIMS_FIELD)
            .to_owned();
        let refs_field = optional_string(evidence.get(evidence_key::REFS_FIELD))
            .unwrap_or(DEFAULT_REFS_FIELD)
            .to_owned();

        let static_refs = string_array(
            evidence.get(evidence_key::ALLOWED_REFS),
            field::EVIDENCE_ALLOWED_REFS,
        )?;
        if let Some(value) = evidence.get(evidence_key::ALLOWED_REFS_FROM_CONTEXT) {
            if !value.is_null() && !value.is_string() {
                return Err(DomainError::InvalidCharacters {
                    field: field::EVIDENCE_CONTEXT_KEY,
                });
            }
        }
        let context_key = optional_string(evidence.get(evidence_key::ALLOWED_REFS_FROM_CONTEXT))
            .map(str::to_owned);

        if static_refs.is_empty() && context_key.is_none() {
            return Err(DomainError::EmptyField {
                field: field::EVIDENCE_ALLOWED_REFS,
            });
        }

        let semantic = semantic_support_spec(evidence.get(evidence_key::SEMANTIC_SUPPORT))?;

        Ok(Some(EvidenceGroundingSpec {
            claims_field,
            refs_field,
            static_refs,
            context_key,
            semantic,
        }))
    }
}

/// Parse the optional `semantic_support` block. Absent/null → `None`;
/// anything but an object with the recognised keys is rejected.
fn semantic_support_spec(
    value: Option<&Value>,
) -> Result<Option<SemanticSupportSpec>, DomainError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(block) = value.as_object() else {
        return Err(DomainError::InvalidCharacters {
            field: field::EVIDENCE_SEMANTIC,
        });
    };

    let known = [
        semantic_key::MIN_CONFIDENCE,
        semantic_key::BODIES_FROM_CONTEXT,
    ];
    if block.keys().any(|key| !known.contains(&key.as_str())) {
        return Err(DomainError::InvalidCharacters {
            field: field::EVIDENCE_SEMANTIC,
        });
    }

    let min_confidence = match block.get(semantic_key::MIN_CONFIDENCE) {
        None => None,
        Some(value) if value.is_null() => None,
        Some(value) => {
            let raw = value.as_u64().ok_or(DomainError::InvalidCharacters {
                field: field::SEMANTIC_MIN_CONFIDENCE,
            })?;
            let percent = u8::try_from(raw).map_err(|_| DomainError::OutOfRange {
                field: field::SEMANTIC_MIN_CONFIDENCE,
                value: raw as f64,
                min: 0.0,
                max: 100.0,
            })?;
            if percent > 100 {
                return Err(DomainError::OutOfRange {
                    field: field::SEMANTIC_MIN_CONFIDENCE,
                    value: f64::from(percent),
                    min: 0.0,
                    max: 100.0,
                });
            }
            Some(percent)
        }
    };

    if let Some(value) = block.get(semantic_key::BODIES_FROM_CONTEXT) {
        if !value.is_null() && !value.is_string() {
            return Err(DomainError::InvalidCharacters {
                field: field::SEMANTIC_BODIES_KEY,
            });
        }
    }
    let bodies_context_key =
        optional_string(block.get(semantic_key::BODIES_FROM_CONTEXT)).map(str::to_owned);

    Ok(Some(SemanticSupportSpec {
        min_confidence,
        bodies_context_key,
    }))
}
