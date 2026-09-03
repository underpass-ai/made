use async_trait::async_trait;
use made_core::entities::{TaskConstraints, ValidatorReport};
use made_core::error::DomainError;
use made_core::ports::ValidatorPort;
use made_core::value_objects::Attributes;
use serde_json::{json, Value};

use super::claim_validation::claim_violations;
use super::json_validation::{attributes, parse_json_object};

#[derive(Debug, Default, Clone, Copy)]
pub struct ClaimsEvidenceGroundedValidator;

/// A validator that enforces the evidence-grounding rule declared in
/// the output contract: every claim in the output must cite at least
/// one evidence reference, and every cited reference must exist in the
/// contract's allowed evidence pack.
///
/// Expected output shape (field names configurable per rule):
///
/// ```json
/// { "claims": [ { "text": "…", "evidence_refs": ["ev-1"] }, … ] }
/// ```
///
/// Semantics:
///
/// - no contract, or contract without a grounding rule → pass
///   (`"no evidence grounding configured"`, the sibling validators'
///   pattern).
/// - claims field missing or not an array → fail (a grounding-gated
///   step must produce inspectable claims).
/// - an empty claims array passes: grounding judges what is claimed,
///   not how much — pair with `required_fields`/`json_schema`
///   (`minItems`) to demand substance.
/// - each claim must be a JSON object whose refs field is a non-empty
///   array of strings, all present in the allowed pack. Violations
///   name the claim index, a text preview and the orphan refs — that
///   detail lands in spans/logs and becomes part of the decision
///   record.
impl ClaimsEvidenceGroundedValidator {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidatorPort for ClaimsEvidenceGroundedValidator {
    fn kind(&self) -> &'static str {
        "claims-evidence-grounded"
    }

    async fn validate(
        &self,
        proposal_content: &str,
        constraints: &TaskConstraints,
    ) -> Result<ValidatorReport, DomainError> {
        let Some(contract) = constraints.output_contract() else {
            return ValidatorReport::new(
                self.kind(),
                true,
                "no evidence grounding configured",
                Attributes::empty(),
            );
        };
        let Some(rule) = contract.evidence_grounding() else {
            return ValidatorReport::new(
                self.kind(),
                true,
                "no evidence grounding configured",
                Attributes::empty(),
            );
        };

        let object = match parse_json_object(proposal_content) {
            Ok(object) => object,
            Err(summary) => {
                return ValidatorReport::new(
                    self.kind(),
                    false,
                    summary,
                    attributes(json!({ "contract_id": contract.contract_id() }))?,
                );
            }
        };

        let Some(claims) = object.get(rule.claims_field()).and_then(Value::as_array) else {
            return ValidatorReport::new(
                self.kind(),
                false,
                format!(
                    "claims field `{}` is missing or not an array",
                    rule.claims_field()
                ),
                attributes(json!({
                    "contract_id": contract.contract_id(),
                    "claims_field": rule.claims_field(),
                }))?,
            );
        };

        let violations: Vec<Value> = claims
            .iter()
            .enumerate()
            .flat_map(|(index, claim)| claim_violations(index, claim, rule))
            .collect();

        if violations.is_empty() {
            ValidatorReport::new(
                self.kind(),
                true,
                format!(
                    "all {} claims grounded in the evidence pack ({} allowed refs)",
                    claims.len(),
                    rule.allowed_refs().len()
                ),
                Attributes::empty(),
            )
        } else {
            ValidatorReport::new(
                self.kind(),
                false,
                format!(
                    "{} of {} claims lack grounded evidence",
                    violations.len(),
                    claims.len()
                ),
                attributes(json!({
                    "contract_id": contract.contract_id(),
                    "claims_field": rule.claims_field(),
                    "refs_field": rule.refs_field(),
                    "violations": violations,
                }))?,
            )
        }
    }
}
