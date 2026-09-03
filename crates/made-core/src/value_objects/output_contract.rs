//! Structured output contract for a council invocation.
//!
//! This is intentionally generic and domain-agnostic. It does not know
//! what a "decision", "report", or "event" means; it only describes
//! the shape that a proposal must satisfy when a caller requires a
//! structured output instead of free-form text.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::output_contract_validation::{
    normalize_optional_schema, validate_text, MAX_FIELDS, MAX_FIELD_NAME_LEN,
};
use crate::error::DomainError;
use crate::value_objects::{
    EvidenceGroundingRule, OutputContractId, OutputFieldRule, OutputFormat,
};

/// Typed structured-output contract attached to one invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputContract {
    contract_id: OutputContractId,
    format: OutputFormat,
    #[serde(default)]
    fields: BTreeMap<String, OutputFieldRule>,
    /// Optional embedded JSON Schema. When non-empty, the adapter
    /// JSON-schema validator parses it once and validates every
    /// proposal output against it in addition to the field-level
    /// rules. Kept as a `String` here so the core stays free of any
    /// schema-engine dependency.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    json_schema: String,
    /// Optional evidence-grounding rule. When present, the adapter
    /// grounding validator rejects proposals whose claims do not cite
    /// evidence from the allowed pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    evidence_grounding: Option<EvidenceGroundingRule>,
}

impl OutputContract {
    pub fn new(
        contract_id: impl Into<String>,
        format: OutputFormat,
        fields: BTreeMap<String, OutputFieldRule>,
    ) -> Result<Self, DomainError> {
        Self::new_with_schema(contract_id, format, fields, String::new())
    }

    /// Build a contract that also carries an embedded JSON Schema
    /// body. The schema text is whitespace-trimmed and length-bounded
    /// (`MAX_JSON_SCHEMA_LEN = 256 KiB`); validation that the body is
    /// itself well-formed JSON / a valid JSON Schema document happens
    /// at adapter wiring time (the core does not pull a schema
    /// engine in).
    pub fn new_with_schema(
        contract_id: impl Into<String>,
        format: OutputFormat,
        fields: BTreeMap<String, OutputFieldRule>,
        json_schema: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let contract_id = OutputContractId::new(contract_id)?;
        if fields.len() > MAX_FIELDS {
            return Err(DomainError::OutOfRange {
                field: "output_contract.fields",
                value: fields.len() as f64,
                min: 0.0,
                max: MAX_FIELDS as f64,
            });
        }

        let mut normalized = BTreeMap::new();
        for (name, rule) in fields {
            let field_name =
                validate_text(&name, "output_contract.field.name", MAX_FIELD_NAME_LEN)?;
            normalized.insert(field_name, rule);
        }

        let json_schema = normalize_optional_schema(&json_schema.into())?;

        Ok(Self {
            contract_id,
            format,
            fields: normalized,
            json_schema,
            evidence_grounding: None,
        })
    }

    pub fn json_object(
        contract_id: impl Into<String>,
        fields: BTreeMap<String, OutputFieldRule>,
    ) -> Result<Self, DomainError> {
        Self::new(contract_id, OutputFormat::JsonObject, fields)
    }

    #[must_use]
    pub const fn contract_id(&self) -> &OutputContractId {
        &self.contract_id
    }

    #[must_use]
    pub const fn format(&self) -> OutputFormat {
        self.format
    }

    #[must_use]
    pub fn fields(&self) -> &BTreeMap<String, OutputFieldRule> {
        &self.fields
    }

    /// Embedded JSON Schema body. Empty string means "no schema —
    /// only field-level rules apply"; the JSON Schema validator
    /// adapter treats empty as a no-op.
    #[must_use]
    pub fn json_schema(&self) -> &str {
        &self.json_schema
    }

    /// Attach an evidence-grounding rule to this contract.
    #[must_use]
    pub fn with_evidence_grounding(mut self, rule: EvidenceGroundingRule) -> Self {
        self.evidence_grounding = Some(rule);
        self
    }

    /// Evidence-grounding rule, when the contract declares one. `None`
    /// means the grounding validator is a no-op for this invocation.
    #[must_use]
    pub fn evidence_grounding(&self) -> Option<&EvidenceGroundingRule> {
        self.evidence_grounding.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::super::output_contract_validation::MAX_JSON_SCHEMA_LEN;
    use super::*;
    use crate::value_objects::SemanticSupportRule;

    fn sample_rule() -> OutputFieldRule {
        OutputFieldRule::new(true, ["emit_event", "escalate"]).unwrap()
    }

    #[test]
    fn json_object_contract_keeps_fields() {
        let contract = OutputContract::json_object(
            "decision-contract",
            BTreeMap::from([("decision".to_owned(), sample_rule())]),
        )
        .unwrap();

        assert_eq!(contract.contract_id(), "decision-contract");
        assert_eq!(contract.format(), OutputFormat::JsonObject);
        assert!(contract.fields()["decision"].required());
        assert!(contract.fields()["decision"]
            .allowed_string_values()
            .contains("emit_event"));
    }

    #[test]
    fn blank_contract_id_is_rejected() {
        let err = OutputContract::json_object("   ", BTreeMap::new()).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "output_contract.contract_id"
            }
        ));
    }

    #[test]
    fn blank_field_name_is_rejected() {
        let err = OutputContract::json_object(
            "c1",
            BTreeMap::from([("   ".to_owned(), OutputFieldRule::default())]),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "output_contract.field.name"
            }
        ));
    }

    #[test]
    fn blank_allowed_value_is_rejected() {
        let err = OutputFieldRule::new(false, [" "]).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "output_contract.field.allowed_value"
            }
        ));
    }

    #[test]
    fn serde_roundtrip_is_stable() {
        let contract = OutputContract::json_object(
            "decision-contract",
            BTreeMap::from([("decision".to_owned(), sample_rule())]),
        )
        .unwrap();
        let serialized = serde_json::to_string(&contract).unwrap();
        let back: OutputContract = serde_json::from_str(&serialized).unwrap();
        assert_eq!(back, contract);
    }

    #[test]
    fn json_schema_is_empty_by_default() {
        let contract = OutputContract::json_object("c1", BTreeMap::new()).unwrap();
        assert!(contract.json_schema().is_empty());
    }

    #[test]
    fn new_with_schema_carries_trimmed_body() {
        let raw = "  { \"type\": \"object\" }  ";
        let contract = OutputContract::new_with_schema(
            "decision-contract",
            OutputFormat::JsonObject,
            BTreeMap::new(),
            raw,
        )
        .unwrap();
        assert_eq!(contract.json_schema(), "{ \"type\": \"object\" }");
    }

    #[test]
    fn overlong_schema_is_rejected() {
        let body = "x".repeat(MAX_JSON_SCHEMA_LEN + 1);
        let err =
            OutputContract::new_with_schema("c1", OutputFormat::JsonObject, BTreeMap::new(), body)
                .unwrap_err();
        assert!(matches!(
            err,
            DomainError::FieldTooLong {
                field: "output_contract.json_schema",
                ..
            }
        ));
    }

    #[test]
    fn evidence_grounding_rule_keeps_fields_and_refs() {
        let rule = EvidenceGroundingRule::new("claims", "evidence_refs", ["ev-1", "ev-2"]).unwrap();
        assert_eq!(rule.claims_field(), "claims");
        assert_eq!(rule.refs_field(), "evidence_refs");
        assert!(rule.allowed_refs().contains("ev-1"));
        assert_eq!(rule.allowed_refs().len(), 2);
    }

    #[test]
    fn evidence_grounding_rule_rejects_empty_pack() {
        let err = EvidenceGroundingRule::new("claims", "evidence_refs", Vec::<String>::new())
            .unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "output_contract.evidence.allowed_refs"
            }
        ));
    }

    #[test]
    fn evidence_grounding_rule_rejects_blank_ref() {
        let err = EvidenceGroundingRule::new("claims", "evidence_refs", ["  "]).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "output_contract.evidence.allowed_ref"
            }
        ));
    }

    #[test]
    fn contract_with_evidence_grounding_roundtrips() {
        let contract = OutputContract::json_object("c1", BTreeMap::new())
            .unwrap()
            .with_evidence_grounding(
                EvidenceGroundingRule::new("claims", "evidence_refs", ["ev-1"]).unwrap(),
            );
        let serialized = serde_json::to_string(&contract).unwrap();
        let back: OutputContract = serde_json::from_str(&serialized).unwrap();
        assert_eq!(back, contract);
        assert_eq!(back.evidence_grounding().unwrap().claims_field(), "claims");
    }

    #[test]
    fn semantic_support_rule_keeps_bodies_and_threshold() {
        let rule =
            SemanticSupportRule::new(80, [("ev-1", "typha held port 5473"), ("ev-2", "crun log")])
                .unwrap();
        assert_eq!(rule.min_confidence(), 80);
        assert_eq!(rule.body("ev-1"), Some("typha held port 5473"));
        assert_eq!(rule.bodies().len(), 2);
    }

    #[test]
    fn semantic_support_rule_rejects_out_of_range_confidence() {
        let err = SemanticSupportRule::new(101, [("ev-1", "body")]).unwrap_err();
        assert!(matches!(
            err,
            DomainError::OutOfRange {
                field: "output_contract.evidence.semantic_support.min_confidence",
                ..
            }
        ));
    }

    #[test]
    fn semantic_support_rule_rejects_empty_bodies() {
        let err = SemanticSupportRule::new(70, Vec::<(String, String)>::new()).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "output_contract.evidence.semantic_support.bodies"
            }
        ));
    }

    #[test]
    fn semantic_support_rule_rejects_blank_body() {
        let err = SemanticSupportRule::new(70, [("ev-1", "   ")]).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "output_contract.evidence.semantic_support.body"
            }
        ));
    }

    #[test]
    fn semantic_support_requires_a_body_for_every_allowed_ref() {
        let grounding =
            EvidenceGroundingRule::new("claims", "evidence_refs", ["ev-1", "ev-2"]).unwrap();
        let partial = SemanticSupportRule::new(70, [("ev-1", "only one body")]).unwrap();
        let err = grounding.with_semantic_support(partial).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "output_contract.evidence.semantic_support.bodies"
            }
        ));
    }

    #[test]
    fn grounding_with_semantic_support_roundtrips() {
        let rule = EvidenceGroundingRule::new("claims", "evidence_refs", ["ev-1"])
            .unwrap()
            .with_semantic_support(SemanticSupportRule::new(70, [("ev-1", "body")]).unwrap())
            .unwrap();
        let contract = OutputContract::json_object("c1", BTreeMap::new())
            .unwrap()
            .with_evidence_grounding(rule);
        let serialized = serde_json::to_string(&contract).unwrap();
        let back: OutputContract = serde_json::from_str(&serialized).unwrap();
        assert_eq!(back, contract);
        let support = back
            .evidence_grounding()
            .unwrap()
            .semantic_support()
            .unwrap();
        assert_eq!(support.min_confidence(), 70);
        assert_eq!(support.body("ev-1"), Some("body"));
    }

    #[test]
    fn grounding_without_semantic_support_deserializes_from_legacy_wire_shape() {
        // Grounding rules serialized before the semantic-support field
        // existed must keep deserializing.
        let legacy =
            r#"{"claims_field":"claims","refs_field":"evidence_refs","allowed_refs":["ev-1"]}"#;
        let back: EvidenceGroundingRule = serde_json::from_str(legacy).unwrap();
        assert!(back.semantic_support().is_none());
    }

    #[test]
    fn contract_without_grounding_deserializes_from_legacy_wire_shape() {
        // Contracts serialized before the grounding field existed must
        // keep deserializing (registry/persistence compatibility).
        let legacy = r#"{"contract_id":"c1","format":"JsonObject","fields":{}}"#;
        let back: OutputContract = serde_json::from_str(legacy).unwrap();
        assert!(back.evidence_grounding().is_none());
    }

    #[test]
    fn schema_serde_roundtrip_preserves_body() {
        let contract = OutputContract::new_with_schema(
            "c1",
            OutputFormat::JsonObject,
            BTreeMap::new(),
            "{\"type\":\"object\"}",
        )
        .unwrap();
        let serialized = serde_json::to_string(&contract).unwrap();
        let back: OutputContract = serde_json::from_str(&serialized).unwrap();
        assert_eq!(back, contract);
        assert_eq!(back.json_schema(), "{\"type\":\"object\"}");
    }
}
