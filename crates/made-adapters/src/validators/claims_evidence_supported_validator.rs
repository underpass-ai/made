use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{TaskConstraints, ValidatorReport};
use made_core::error::DomainError;
use made_core::ports::{EvidenceSupportJudgePort, ValidatorPort};
use made_core::value_objects::{
    Attributes, ClaimText, EvidenceBody, EvidenceExcerpt, EvidenceReference,
};
use serde_json::{json, Map, Value};

use super::claim_validation::claim_preview;
use super::json_validation::{attributes, parse_json_object, value_type_name};

pub struct ClaimsEvidenceSupportedValidator {
    pub(super) judge: Option<Arc<dyn EvidenceSupportJudgePort>>,
}

/// Cap on the number of claims one proposal may put in front of the
/// support judge. Far above any real decision output; a proposal that
/// exceeds it fails the gate instead of burning a judge call per claim.
const MAX_JUDGED_CLAIMS: usize = 64;

/// A validator that enforces the semantic-support rule declared in the
/// output contract's evidence block: every claim's *cited* evidence
/// bodies must actually support what the claim says, as judged through
/// the wired [`EvidenceSupportJudgePort`].
///
/// This is the second gate behind [`super::ClaimsEvidenceGroundedValidator`]:
/// grounding proves the citation exists; this proves it holds. The
/// judgment is model-backed, but the *decision* stays deterministic —
/// a claim passes iff the verdict says `supported` with confidence at
/// or above the contract's `min_confidence` — and every verdict
/// (supported or not, with its rationale) is recorded in the report's
/// details, so the judge's opinion becomes part of the decision record
/// instead of an unrecorded opinion.
///
/// Semantics:
///
/// - no contract, or no grounding rule, or no semantic-support rule →
///   pass (`"no semantic support configured"`, the sibling validators'
///   pattern).
/// - semantic-support rule declared but no judge wired → **hard
///   error**: running the step unjudged would silently void the
///   policy, the same posture the grounding gate takes on an absent
///   evidence pack.
/// - a judge transport/parse failure is a hard error too (fail
///   closed): a gate that cannot judge must not wave proposals
///   through.
/// - claims citing refs the rule has no body for produce a violation
///   (those refs are outside the pack — the grounding gate names them;
///   this gate refuses to judge on nothing).
impl std::fmt::Debug for ClaimsEvidenceSupportedValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaimsEvidenceSupportedValidator")
            .field("judge_wired", &self.judge.is_some())
            .finish()
    }
}

impl ClaimsEvidenceSupportedValidator {
    /// Build the validator. `judge` is `None` when the deployment did
    /// not wire a support judge; the validator stays a no-op unless a
    /// contract demands semantic support, in which case it fails
    /// loudly.
    #[must_use]
    pub fn new(judge: Option<Arc<dyn EvidenceSupportJudgePort>>) -> Self {
        Self { judge }
    }
}

#[async_trait]
impl ValidatorPort for ClaimsEvidenceSupportedValidator {
    fn kind(&self) -> &'static str {
        "claims-evidence-supported"
    }

    async fn validate(
        &self,
        proposal_content: &str,
        constraints: &TaskConstraints,
    ) -> Result<ValidatorReport, DomainError> {
        let Some(rule) = constraints
            .output_contract()
            .and_then(|contract| contract.evidence_grounding())
        else {
            return ValidatorReport::new(
                self.kind(),
                true,
                "no semantic support configured",
                Attributes::empty(),
            );
        };
        let Some(support) = rule.semantic_support() else {
            return ValidatorReport::new(
                self.kind(),
                true,
                "no semantic support configured",
                Attributes::empty(),
            );
        };
        let Some(judge) = self.judge.as_ref() else {
            // Fail the step, not the proposal: a contract demanding a
            // judgment the deployment cannot produce is a wiring gap,
            // and waving the proposal through would void the policy.
            return Err(DomainError::InvariantViolated {
                reason: "semantic support demanded by the contract but no \
                         evidence-support judge is wired (set MADE_SUPPORT_JUDGE_ENABLED)",
            });
        };
        // `unwrap` is safe: the two `let Some` above prove the contract exists.
        let contract_id = constraints
            .output_contract()
            .map(made_core::value_objects::OutputContract::contract_id)
            .map_or_else(String::new, |id| id.as_str().to_owned());

        let object = match parse_json_object(proposal_content) {
            Ok(object) => object,
            Err(summary) => {
                return ValidatorReport::new(
                    self.kind(),
                    false,
                    summary,
                    attributes(json!({ "contract_id": contract_id }))?,
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
                    "contract_id": contract_id,
                    "claims_field": rule.claims_field(),
                }))?,
            );
        };
        if claims.len() > MAX_JUDGED_CLAIMS {
            return ValidatorReport::new(
                self.kind(),
                false,
                format!(
                    "{} claims exceed the judgeable limit of {MAX_JUDGED_CLAIMS}",
                    claims.len()
                ),
                attributes(json!({ "contract_id": contract_id }))?,
            );
        }

        let (verdicts, violations) =
            judge_claims(judge.as_ref(), claims, rule.refs_field(), support).await?;

        let details = attributes(json!({
            "contract_id": contract_id,
            "min_confidence": support.min_confidence(),
            "verdicts": verdicts,
            "violations": violations,
        }))?;
        if violations.is_empty() {
            ValidatorReport::new(
                self.kind(),
                true,
                format!(
                    "all {} claims semantically supported by their cited evidence \
                     (min_confidence {})",
                    claims.len(),
                    support.min_confidence()
                ),
                details,
            )
        } else {
            ValidatorReport::new(
                self.kind(),
                false,
                format!(
                    "{} of {} claims lack semantic support from their cited evidence",
                    violations.len(),
                    claims.len()
                ),
                details,
            )
        }
    }
}

/// Judge every claim and collect the verdicts (all of them, so the
/// judge's opinion rides the decision record) plus the violations (the
/// claims the deterministic rule rejects). A judge failure propagates:
/// the gate fails closed.
async fn judge_claims(
    judge: &dyn EvidenceSupportJudgePort,
    claims: &[Value],
    refs_field: &str,
    support: &made_core::value_objects::SemanticSupportRule,
) -> Result<(Vec<Value>, Vec<Value>), DomainError> {
    let mut verdicts = Vec::with_capacity(claims.len());
    let mut violations = Vec::new();
    for (index, claim) in claims.iter().enumerate() {
        let Some(claim_object) = claim.as_object() else {
            violations.push(json!({
                "claim_index": index,
                "problem": "claim is not a JSON object",
                "actual_type": value_type_name(claim),
            }));
            continue;
        };
        let preview = claim_preview(claim_object);
        let excerpts = cited_excerpts(claim_object, refs_field, support);
        if excerpts.is_empty() {
            violations.push(json!({
                "claim_index": index,
                "claim_preview": preview,
                "problem": "claim cites no evidence with a judgeable body",
            }));
            continue;
        }
        let claim_text = claim_object
            .get("text")
            .and_then(Value::as_str)
            .map_or_else(
                || Value::Object(claim_object.clone()).to_string(),
                str::to_owned,
            );

        let claim_text = ClaimText::new(claim_text)?;
        let verdict = judge.assess(&claim_text, &excerpts).await?;
        let accepted = verdict.decision().is_supported()
            && verdict.confidence().meets(support.min_confidence());
        verdicts.push(json!({
            "claim_index": index,
            "claim_preview": preview,
            "refs": excerpts
                .iter()
                .map(|excerpt| excerpt.reference().as_str())
                .collect::<Vec<_>>(),
            "supported": verdict.decision().is_supported(),
            "confidence": verdict.confidence().percent(),
            "rationale": verdict.rationale().as_str(),
        }));
        if !accepted {
            violations.push(json!({
                "claim_index": index,
                "claim_preview": preview,
                "problem": if verdict.decision().is_supported() {
                    format!(
                        "supported but confidence {} is below min_confidence {}",
                        verdict.confidence().percent(),
                        support.min_confidence()
                    )
                } else {
                    "cited evidence does not support the claim".to_owned()
                },
            }));
        }
    }
    Ok((verdicts, violations))
}

/// The evidence excerpts a claim actually cited, resolved to bodies.
/// Refs without a body in the rule are skipped — they are outside the
/// pack, which the grounding gate reports by name; this gate only
/// judges on real text.
fn cited_excerpts(
    claim: &Map<String, Value>,
    refs_field: &str,
    support: &made_core::value_objects::SemanticSupportRule,
) -> Vec<EvidenceExcerpt> {
    claim
        .get(refs_field)
        .and_then(Value::as_array)
        .map(|refs| {
            refs.iter()
                .filter_map(Value::as_str)
                .filter_map(|reference| {
                    support.body(reference).and_then(|body| {
                        Some(EvidenceExcerpt::new(
                            EvidenceReference::new(reference).ok()?,
                            EvidenceBody::new(body).ok()?,
                        ))
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
