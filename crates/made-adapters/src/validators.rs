//! Default validator adapters.
//!
//! These implementations cover use-case-agnostic sanity checks. They
//! are deliberately minimal; domain-specific validators (clinical
//! safety, policy compliance, fact checking, …) belong in the
//! integrating product, not in MADE.

mod allowed_string_values_validator;
mod bounded_event_shape_validator;
mod claim_validation;
mod claims_evidence_grounded_validator;
mod claims_evidence_supported_validator;
mod content_non_empty_validator;
mod json_object_output_validator;
mod json_schema_validator;
mod json_validation;
mod required_fields_validator;
mod shape_violation;

pub use allowed_string_values_validator::AllowedStringValuesValidator;
pub use bounded_event_shape_validator::BoundedEventShapeValidator;
pub use claims_evidence_grounded_validator::ClaimsEvidenceGroundedValidator;
pub use claims_evidence_supported_validator::ClaimsEvidenceSupportedValidator;
pub use content_non_empty_validator::ContentNonEmptyValidator;
pub use json_object_output_validator::JsonObjectOutputValidator;
pub use json_schema_validator::JsonSchemaValidator;
pub use required_fields_validator::RequiredFieldsValidator;

#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use async_trait::async_trait;
#[cfg(test)]
use json_validation::strip_markdown_fences;
#[cfg(test)]
use made_core::entities::TaskConstraints;
#[cfg(test)]
use made_core::error::DomainError;
#[cfg(test)]
use made_core::ports::{EvidenceSupportJudgePort, ValidatorPort};
#[cfg(test)]
use made_core::value_objects::{ClaimText, EvidenceExcerpt};
#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{OutputContract, OutputFieldRule};
    use std::collections::BTreeMap;

    fn structured_constraints() -> TaskConstraints {
        TaskConstraints::default().with_output_contract(
            OutputContract::json_object(
                "decision-contract",
                BTreeMap::from([
                    (
                        "decision".to_owned(),
                        OutputFieldRule::new(true, ["emit_event", "escalate"]).unwrap(),
                    ),
                    (
                        "reason".to_owned(),
                        OutputFieldRule::new(true, std::iter::empty::<&str>()).unwrap(),
                    ),
                ]),
            )
            .unwrap(),
        )
    }

    fn grounded_constraints() -> TaskConstraints {
        TaskConstraints::default().with_output_contract(
            OutputContract::json_object("evidence-contract", BTreeMap::new())
                .unwrap()
                .with_evidence_grounding(
                    made_core::value_objects::EvidenceGroundingRule::new(
                        "claims",
                        "evidence_refs",
                        ["ev-1", "ev-2"],
                    )
                    .unwrap(),
                ),
        )
    }

    #[tokio::test]
    async fn grounding_passes_without_configuration() {
        let v = ClaimsEvidenceGroundedValidator::new();
        let r = v
            .validate("free prose", &TaskConstraints::default())
            .await
            .unwrap();
        assert!(r.passed());
        assert_eq!(r.kind(), "claims-evidence-grounded");
        assert!(r.summary().contains("no evidence grounding configured"));

        // A contract without a grounding rule is also a no-op.
        let r = v
            .validate("free prose", &structured_constraints())
            .await
            .unwrap();
        assert!(r.passed());
    }

    #[test]
    fn strip_markdown_fences_unwraps_pure_fenced_block() {
        assert_eq!(
            strip_markdown_fences("```json\n{\"a\": 1}\n```"),
            "{\"a\": 1}"
        );
        assert_eq!(strip_markdown_fences("```\n{\"a\": 1}\n```"), "{\"a\": 1}");
        // Sin fences: intacto.
        assert_eq!(strip_markdown_fences("  {\"a\": 1} "), "{\"a\": 1}");
        // Prosa alrededor del fence: intacto (fallara el parse, a proposito).
        let mixed = "look: ```json\n{}\n```";
        assert_eq!(strip_markdown_fences(mixed), mixed.trim());
        // Fence sin cierre: intacto.
        assert_eq!(strip_markdown_fences("```json\n{"), "```json\n{");
    }

    #[tokio::test]
    async fn grounded_claims_pass_inside_markdown_fences() {
        let v = ClaimsEvidenceGroundedValidator::new();
        let content = "```json\n{\"claims\":[{\"text\":\"typha holds the port\",\"evidence_refs\":[\"ev-1\"]}]}\n```";
        let r = v.validate(content, &grounded_constraints()).await.unwrap();
        assert!(r.passed(), "summary: {}", r.summary());
    }

    #[tokio::test]
    async fn grounded_claims_pass() {
        let v = ClaimsEvidenceGroundedValidator::new();
        let content = r#"{"claims":[
            {"text":"typha holds the port","evidence_refs":["ev-1"]},
            {"text":"crun state is per root","evidence_refs":["ev-1","ev-2"]}
        ]}"#;
        let r = v.validate(content, &grounded_constraints()).await.unwrap();
        assert!(r.passed(), "summary: {}", r.summary());
        assert!(r.summary().contains("all 2 claims grounded"));
    }

    #[tokio::test]
    async fn claim_without_refs_fails() {
        let v = ClaimsEvidenceGroundedValidator::new();
        let content = r#"{"claims":[
            {"text":"grounded","evidence_refs":["ev-1"]},
            {"text":"vibes only"}
        ]}"#;
        let r = v.validate(content, &grounded_constraints()).await.unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("1 of 2 claims"));
    }

    #[tokio::test]
    async fn claim_with_empty_refs_fails() {
        let v = ClaimsEvidenceGroundedValidator::new();
        let content = r#"{"claims":[{"text":"empty-handed","evidence_refs":[]}]}"#;
        let r = v.validate(content, &grounded_constraints()).await.unwrap();
        assert!(!r.passed());
    }

    #[tokio::test]
    async fn orphan_ref_fails_and_is_named() {
        let v = ClaimsEvidenceGroundedValidator::new();
        let content = r#"{"claims":[{"text":"invented","evidence_refs":["ev-999"]}]}"#;
        let r = v.validate(content, &grounded_constraints()).await.unwrap();
        assert!(!r.passed());
        let details = serde_json::to_string(r.details()).unwrap();
        assert!(details.contains("ev-999"));
    }

    #[tokio::test]
    async fn missing_claims_field_fails() {
        let v = ClaimsEvidenceGroundedValidator::new();
        let r = v
            .validate(r#"{"decision":"accept"}"#, &grounded_constraints())
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("claims"));
    }

    #[tokio::test]
    async fn empty_claims_array_passes() {
        let v = ClaimsEvidenceGroundedValidator::new();
        let r = v
            .validate(r#"{"claims":[]}"#, &grounded_constraints())
            .await
            .unwrap();
        assert!(r.passed());
    }

    // ---- ClaimsEvidenceSupportedValidator ------------------------------

    use made_core::value_objects::{
        SemanticSupportRule, SupportConfidence, SupportDecision, SupportRationale, SupportVerdict,
    };

    /// Scripted judge: answers by looking the claim text up in a
    /// verdict table; records what it was asked so tests can assert the
    /// excerpts a claim put in front of it.
    #[derive(Default)]
    struct ScriptedJudge {
        verdicts: std::collections::BTreeMap<String, SupportVerdict>,
        asked: std::sync::Mutex<Vec<(String, Vec<String>)>>,
    }

    #[async_trait]
    impl EvidenceSupportJudgePort for ScriptedJudge {
        async fn assess(
            &self,
            claim: &ClaimText,
            evidence: &[EvidenceExcerpt],
        ) -> Result<SupportVerdict, DomainError> {
            self.asked.lock().unwrap().push((
                claim.as_str().to_owned(),
                evidence
                    .iter()
                    .map(|excerpt| excerpt.reference().as_str().to_owned())
                    .collect(),
            ));
            self.verdicts
                .get(claim.as_str())
                .cloned()
                .ok_or(DomainError::InvariantViolated {
                    reason: "scripted judge: unexpected claim",
                })
        }
    }

    fn verdict(supported: bool, confidence: u8) -> SupportVerdict {
        SupportVerdict::new(
            SupportDecision::from(supported),
            SupportConfidence::new(confidence).unwrap(),
            SupportRationale::new("scripted"),
        )
    }

    fn supported_constraints(min_confidence: u8) -> TaskConstraints {
        let rule = made_core::value_objects::EvidenceGroundingRule::new(
            "claims",
            "evidence_refs",
            ["ev-1", "ev-2"],
        )
        .unwrap()
        .with_semantic_support(
            SemanticSupportRule::new(
                min_confidence,
                [
                    ("ev-1", "journalctl: typha (pid 4830) holds 0.0.0.0:5473"),
                    ("ev-2", "crun state dir is scoped per runtime root"),
                ],
            )
            .unwrap(),
        )
        .unwrap();
        TaskConstraints::default().with_output_contract(
            OutputContract::json_object("support-contract", BTreeMap::new())
                .unwrap()
                .with_evidence_grounding(rule),
        )
    }

    #[tokio::test]
    async fn support_validator_passes_without_configuration() {
        let v = ClaimsEvidenceSupportedValidator::new(None);
        // No contract at all.
        let r = v
            .validate("free prose", &TaskConstraints::default())
            .await
            .unwrap();
        assert!(r.passed());
        assert_eq!(r.kind(), "claims-evidence-supported");
        assert!(r.summary().contains("no semantic support configured"));
        // A grounding rule without a semantic-support rule is also a no-op.
        let r = v
            .validate("free prose", &grounded_constraints())
            .await
            .unwrap();
        assert!(r.passed());
    }

    #[tokio::test]
    async fn support_demanded_without_a_judge_fails_the_step_loudly() {
        let v = ClaimsEvidenceSupportedValidator::new(None);
        let err = v
            .validate(r#"{"claims":[]}"#, &supported_constraints(70))
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::InvariantViolated { reason }
            if reason.contains("no evidence-support judge is wired")));
    }

    #[tokio::test]
    async fn supported_claims_pass_and_verdicts_ride_the_details() {
        let judge = ScriptedJudge {
            verdicts: BTreeMap::from([("typha holds the port".to_owned(), verdict(true, 92))]),
            ..Default::default()
        };
        let v = ClaimsEvidenceSupportedValidator::new(Some(Arc::new(judge)));
        let content = r#"{"claims":[{"text":"typha holds the port","evidence_refs":["ev-1"]}]}"#;
        let r = v
            .validate(content, &supported_constraints(70))
            .await
            .unwrap();
        assert!(r.passed(), "summary: {}", r.summary());
        let details = serde_json::to_string(r.details()).unwrap();
        assert!(details.contains("\"supported\":true"));
        assert!(details.contains("\"confidence\":92"));
        assert!(details.contains("scripted"));
    }

    #[tokio::test]
    async fn unsupported_claim_fails_and_is_named() {
        let judge = ScriptedJudge {
            verdicts: BTreeMap::from([
                ("grounded and true".to_owned(), verdict(true, 90)),
                (
                    "cites real refs, says nonsense".to_owned(),
                    verdict(false, 88),
                ),
            ]),
            ..Default::default()
        };
        let v = ClaimsEvidenceSupportedValidator::new(Some(Arc::new(judge)));
        let content = r#"{"claims":[
            {"text":"grounded and true","evidence_refs":["ev-1"]},
            {"text":"cites real refs, says nonsense","evidence_refs":["ev-1","ev-2"]}
        ]}"#;
        let r = v
            .validate(content, &supported_constraints(70))
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("1 of 2 claims"));
        let details = serde_json::to_string(r.details()).unwrap();
        assert!(details.contains("does not support the claim"));
    }

    #[tokio::test]
    async fn low_confidence_support_fails_under_the_threshold() {
        let judge = ScriptedJudge {
            verdicts: BTreeMap::from([("maybe".to_owned(), verdict(true, 55))]),
            ..Default::default()
        };
        let v = ClaimsEvidenceSupportedValidator::new(Some(Arc::new(judge)));
        let content = r#"{"claims":[{"text":"maybe","evidence_refs":["ev-1"]}]}"#;
        let r = v
            .validate(content, &supported_constraints(70))
            .await
            .unwrap();
        assert!(!r.passed());
        let details = serde_json::to_string(r.details()).unwrap();
        assert!(details.contains("below min_confidence"));
    }

    #[tokio::test]
    async fn judge_only_sees_the_evidence_the_claim_cited() {
        let judge = Arc::new(ScriptedJudge {
            verdicts: BTreeMap::from([("narrow".to_owned(), verdict(true, 95))]),
            ..Default::default()
        });
        let v = ClaimsEvidenceSupportedValidator::new(Some(judge.clone()));
        let content = r#"{"claims":[{"text":"narrow","evidence_refs":["ev-2"]}]}"#;
        v.validate(content, &supported_constraints(70))
            .await
            .unwrap();
        let asked = judge.asked.lock().unwrap();
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].1, vec!["ev-2".to_owned()]);
    }

    #[tokio::test]
    async fn claim_citing_only_orphan_refs_fails_without_a_judge_call() {
        let judge = Arc::new(ScriptedJudge::default());
        let v = ClaimsEvidenceSupportedValidator::new(Some(judge.clone()));
        let content = r#"{"claims":[{"text":"invented","evidence_refs":["ev-999"]}]}"#;
        let r = v
            .validate(content, &supported_constraints(70))
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(judge.asked.lock().unwrap().is_empty());
        let details = serde_json::to_string(r.details()).unwrap();
        assert!(details.contains("no evidence with a judgeable body"));
    }

    #[tokio::test]
    async fn judge_failure_fails_the_step_closed() {
        // Empty verdict table: any claim makes the scripted judge error.
        let judge = Arc::new(ScriptedJudge::default());
        let v = ClaimsEvidenceSupportedValidator::new(Some(judge));
        let content = r#"{"claims":[{"text":"anything","evidence_refs":["ev-1"]}]}"#;
        let err = v
            .validate(content, &supported_constraints(70))
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }

    #[tokio::test]
    async fn empty_claims_array_passes_the_support_gate() {
        let v = ClaimsEvidenceSupportedValidator::new(Some(Arc::new(ScriptedJudge::default())));
        let r = v
            .validate(r#"{"claims":[]}"#, &supported_constraints(70))
            .await
            .unwrap();
        assert!(r.passed());
    }

    #[tokio::test]
    async fn missing_claims_field_fails_the_support_gate() {
        let v = ClaimsEvidenceSupportedValidator::new(Some(Arc::new(ScriptedJudge::default())));
        let r = v
            .validate(r#"{"decision":"accept"}"#, &supported_constraints(70))
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("claims"));
    }

    #[tokio::test]
    async fn non_empty_content_passes() {
        let v = ContentNonEmptyValidator::new();
        let r = v
            .validate("hello", &TaskConstraints::default())
            .await
            .unwrap();
        assert!(r.passed());
        assert_eq!(r.kind(), "content-non-empty");
    }

    #[tokio::test]
    async fn whitespace_only_content_fails() {
        let v = ContentNonEmptyValidator::new();
        let r = v
            .validate("   \n\t ", &TaskConstraints::default())
            .await
            .unwrap();
        assert!(!r.passed());
    }

    #[tokio::test]
    async fn empty_string_fails() {
        let v = ContentNonEmptyValidator::new();
        let r = v.validate("", &TaskConstraints::default()).await.unwrap();
        assert!(!r.passed());
    }

    #[tokio::test]
    async fn json_object_validator_accepts_valid_object() {
        let v = JsonObjectOutputValidator::new();
        let r = v
            .validate(r#"{"decision":"emit_event"}"#, &structured_constraints())
            .await
            .unwrap();
        assert!(r.passed());
    }

    #[tokio::test]
    async fn json_object_validator_rejects_non_json() {
        let v = JsonObjectOutputValidator::new();
        let r = v
            .validate("not json", &structured_constraints())
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("not valid JSON"));
    }

    #[tokio::test]
    async fn required_fields_validator_rejects_missing_fields() {
        let v = RequiredFieldsValidator::new();
        let r = v
            .validate(r#"{"decision":"emit_event"}"#, &structured_constraints())
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("reason"));
    }

    #[tokio::test]
    async fn allowed_string_values_validator_rejects_unknown_value() {
        let v = AllowedStringValuesValidator::new();
        let r = v
            .validate(
                r#"{"decision":"drop_everything","reason":"missing evidence"}"#,
                &structured_constraints(),
            )
            .await
            .unwrap();
        assert!(!r.passed());
        assert_eq!(r.kind(), "output-allowed-string-values");
    }

    #[tokio::test]
    async fn contract_validators_are_noops_without_contract() {
        let constraints = TaskConstraints::default();
        for report in [
            JsonObjectOutputValidator::new()
                .validate("plain text", &constraints)
                .await
                .unwrap(),
            RequiredFieldsValidator::new()
                .validate("plain text", &constraints)
                .await
                .unwrap(),
            AllowedStringValuesValidator::new()
                .validate("plain text", &constraints)
                .await
                .unwrap(),
            JsonSchemaValidator::new()
                .validate("plain text", &constraints)
                .await
                .unwrap(),
        ] {
            assert!(report.passed());
        }
    }

    fn schema_constraints(schema_body: &str) -> TaskConstraints {
        TaskConstraints::default().with_output_contract(
            OutputContract::new_with_schema(
                "schema-contract",
                made_core::value_objects::OutputFormat::JsonObject,
                BTreeMap::new(),
                schema_body,
            )
            .unwrap(),
        )
    }

    #[tokio::test]
    async fn json_schema_validator_is_noop_without_embedded_schema() {
        // structured_constraints has no JSON Schema attached.
        let v = JsonSchemaValidator::new();
        let r = v
            .validate(r#"{"any": "shape"}"#, &structured_constraints())
            .await
            .unwrap();
        assert!(r.passed());
        assert_eq!(r.kind(), "output-json-schema");
        assert!(r.summary().contains("no embedded JSON Schema"));
    }

    #[tokio::test]
    async fn json_schema_validator_accepts_satisfying_output() {
        let schema = r#"{
            "type": "object",
            "required": ["decision", "reason"],
            "properties": {
                "decision": { "type": "string", "enum": ["emit_event", "escalate"] },
                "reason": { "type": "string", "minLength": 1 }
            },
            "additionalProperties": false
        }"#;
        let v = JsonSchemaValidator::new();
        let r = v
            .validate(
                r#"{"decision":"emit_event","reason":"clear signal"}"#,
                &schema_constraints(schema),
            )
            .await
            .unwrap();
        assert!(r.passed(), "summary: {}", r.summary());
    }

    #[tokio::test]
    async fn json_schema_validator_rejects_missing_required_field() {
        let schema = r#"{
            "type": "object",
            "required": ["decision", "reason"],
            "properties": {
                "decision": { "type": "string" },
                "reason": { "type": "string" }
            }
        }"#;
        let v = JsonSchemaValidator::new();
        let r = v
            .validate(r#"{"decision":"emit_event"}"#, &schema_constraints(schema))
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("schema violation"));
    }

    #[tokio::test]
    async fn json_schema_validator_rejects_violation_of_max_items() {
        // bounded shape: maxItems on findings. Subsumes the
        // "bounded event proposal shape" deliverable.
        let schema = r#"{
            "type": "object",
            "properties": {
                "findings": {
                    "type": "array",
                    "maxItems": 2,
                    "items": { "type": "string", "maxLength": 64 }
                }
            }
        }"#;
        let v = JsonSchemaValidator::new();
        let r = v
            .validate(r#"{"findings":["a","b","c"]}"#, &schema_constraints(schema))
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("schema violation"));
    }

    #[tokio::test]
    async fn json_schema_validator_reports_malformed_schema() {
        let v = JsonSchemaValidator::new();
        let r = v
            .validate(
                r#"{"decision":"emit_event"}"#,
                &schema_constraints("not a json schema"),
            )
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("not valid JSON"));
    }

    #[tokio::test]
    async fn json_schema_validator_reports_malformed_proposal() {
        let v = JsonSchemaValidator::new();
        let r = v
            .validate(
                "not json at all",
                &schema_constraints(r#"{"type":"object"}"#),
            )
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("proposal is not valid JSON"));
    }

    // ---- BoundedEventShapeValidator -----------------------------------

    fn unconstrained() -> TaskConstraints {
        TaskConstraints::default()
    }

    #[tokio::test]
    async fn bounded_event_shape_is_noop_without_contract() {
        let v = BoundedEventShapeValidator::new();
        let r = v.validate(r#"{"k":"v"}"#, &unconstrained()).await.unwrap();
        assert!(r.passed());
        assert!(r.summary().contains("no structured output contract"));
    }

    #[tokio::test]
    async fn bounded_event_shape_accepts_modest_payloads() {
        let v = BoundedEventShapeValidator::new();
        let r = v
            .validate(
                r#"{"decision":"emit_event","reason":"clear"}"#,
                &structured_constraints(),
            )
            .await
            .unwrap();
        assert!(r.passed(), "summary: {}", r.summary());
        assert_eq!(r.kind(), "output-bounded-event-shape");
    }

    #[tokio::test]
    async fn bounded_event_shape_rejects_oversized_payloads() {
        let mut bloated = String::from("{\"k\":\"");
        bloated.push_str(&"a".repeat(2048));
        bloated.push_str("\"}");
        let v = BoundedEventShapeValidator::new().with_max_total_size_bytes(512);
        let r = v
            .validate(&bloated, &structured_constraints())
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("limit is 512"));
    }

    #[tokio::test]
    async fn bounded_event_shape_rejects_too_many_object_keys() {
        use std::fmt::Write as _;
        let mut payload = String::from("{");
        for i in 0..40 {
            if i > 0 {
                payload.push(',');
            }
            write!(payload, r#""k{i}":{i}"#).unwrap();
        }
        payload.push('}');
        let v = BoundedEventShapeValidator::new().with_max_object_keys(32);
        let r = v
            .validate(&payload, &structured_constraints())
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("max_object_keys"));
    }

    #[tokio::test]
    async fn bounded_event_shape_rejects_too_deep_nesting() {
        // 6 levels of nesting around a literal.
        let payload = r#"{"a":{"b":{"c":{"d":{"e":{"f":1}}}}}}"#;
        let v = BoundedEventShapeValidator::new().with_max_depth(4);
        let r = v
            .validate(payload, &structured_constraints())
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("max_depth"));
    }

    #[tokio::test]
    async fn bounded_event_shape_rejects_long_strings() {
        let payload = format!(r#"{{"reason":"{}"}}"#, "x".repeat(200));
        let v = BoundedEventShapeValidator::new().with_max_string_len(64);
        let r = v
            .validate(&payload, &structured_constraints())
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("max_string_len"));
    }

    #[tokio::test]
    async fn bounded_event_shape_rejects_huge_arrays() {
        let nums = (0..50).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let payload = format!(r#"{{"items":[{nums}]}}"#);
        let v = BoundedEventShapeValidator::new().with_max_array_len(10);
        let r = v
            .validate(&payload, &structured_constraints())
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("max_array_len"));
    }

    #[tokio::test]
    async fn bounded_event_shape_rejects_invalid_json() {
        let v = BoundedEventShapeValidator::new();
        let r = v
            .validate("not json", &structured_constraints())
            .await
            .unwrap();
        assert!(!r.passed());
        assert!(r.summary().contains("not valid JSON"));
    }
}
