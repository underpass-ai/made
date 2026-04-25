//! Default validator adapters.
//!
//! These implementations cover use-case-agnostic sanity checks. They
//! are deliberately minimal; domain-specific validators (clinical
//! safety, policy compliance, fact checking, …) belong in the
//! integrating product, not in the Choreographer.

use async_trait::async_trait;
use choreo_core::entities::{TaskConstraints, ValidatorReport};
use choreo_core::error::DomainError;
use choreo_core::ports::ValidatorPort;
use choreo_core::value_objects::Attributes;
use serde_json::{json, Map, Value};

/// A validator that fails a proposal only when its content is empty
/// (after trimming). The thinnest possible "is there anything here"
/// check — useful as a default so operators who do not configure a
/// richer validator still get a meaningful ranking signal.
#[derive(Debug, Default, Clone, Copy)]
pub struct ContentNonEmptyValidator;

impl ContentNonEmptyValidator {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidatorPort for ContentNonEmptyValidator {
    fn kind(&self) -> &'static str {
        "content-non-empty"
    }

    async fn validate(
        &self,
        proposal_content: &str,
        _constraints: &TaskConstraints,
    ) -> Result<ValidatorReport, DomainError> {
        let trimmed = proposal_content.trim();
        let passed = !trimmed.is_empty();
        let summary = if passed {
            format!("len={}", trimmed.len())
        } else {
            "content is empty after trimming".to_owned()
        };
        ValidatorReport::new(self.kind(), passed, summary, Attributes::empty())
    }
}

/// A validator that requires structured-output proposals to be valid
/// JSON objects at the root.
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonObjectOutputValidator;

impl JsonObjectOutputValidator {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidatorPort for JsonObjectOutputValidator {
    fn kind(&self) -> &'static str {
        "output-json-object"
    }

    async fn validate(
        &self,
        proposal_content: &str,
        constraints: &TaskConstraints,
    ) -> Result<ValidatorReport, DomainError> {
        if constraints.output_contract().is_none() {
            return ValidatorReport::new(
                self.kind(),
                true,
                "no structured output contract configured",
                Attributes::empty(),
            );
        }

        match parse_json_object(proposal_content) {
            Ok(_) => ValidatorReport::new(
                self.kind(),
                true,
                "proposal is a valid JSON object",
                Attributes::empty(),
            ),
            Err(summary) => ValidatorReport::new(
                self.kind(),
                false,
                summary,
                attributes(json!({ "expected_format": "json_object" }))?,
            ),
        }
    }
}

/// A validator that enforces required fields declared in the output
/// contract.
#[derive(Debug, Default, Clone, Copy)]
pub struct RequiredFieldsValidator;

impl RequiredFieldsValidator {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidatorPort for RequiredFieldsValidator {
    fn kind(&self) -> &'static str {
        "output-required-fields"
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
                "no structured output contract configured",
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

        let missing_fields = contract
            .fields()
            .iter()
            .filter(|(_, rule)| rule.required())
            .filter(|(field_name, _)| !object.contains_key(*field_name))
            .map(|(field_name, _)| field_name.clone())
            .collect::<Vec<_>>();

        if missing_fields.is_empty() {
            ValidatorReport::new(
                self.kind(),
                true,
                "all required fields are present",
                Attributes::empty(),
            )
        } else {
            ValidatorReport::new(
                self.kind(),
                false,
                format!("missing required fields: {}", missing_fields.join(", ")),
                attributes(json!({
                    "contract_id": contract.contract_id(),
                    "missing_fields": missing_fields,
                }))?,
            )
        }
    }
}

/// A validator that enforces allowed string sets declared for one or
/// more fields in the output contract.
#[derive(Debug, Default, Clone, Copy)]
pub struct AllowedStringValuesValidator;

impl AllowedStringValuesValidator {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidatorPort for AllowedStringValuesValidator {
    fn kind(&self) -> &'static str {
        "output-allowed-string-values"
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
                "no structured output contract configured",
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

        let mut violations = Vec::new();
        for (field_name, rule) in contract.fields() {
            if rule.allowed_string_values().is_empty() {
                continue;
            }
            let Some(value) = object.get(field_name) else {
                continue;
            };
            match value {
                Value::String(actual) if rule.allowed_string_values().contains(actual) => {}
                Value::String(actual) => violations.push(json!({
                    "field": field_name,
                    "actual": actual,
                    "allowed": rule.allowed_string_values(),
                })),
                other => violations.push(json!({
                    "field": field_name,
                    "actual_type": value_type_name(other),
                    "allowed": rule.allowed_string_values(),
                })),
            }
        }

        if violations.is_empty() {
            ValidatorReport::new(
                self.kind(),
                true,
                "all constrained string fields satisfy their allowed value sets",
                Attributes::empty(),
            )
        } else {
            ValidatorReport::new(
                self.kind(),
                false,
                "one or more constrained string fields violated the contract",
                attributes(json!({
                    "contract_id": contract.contract_id(),
                    "violations": violations,
                }))?,
            )
        }
    }
}

fn parse_json_object(proposal_content: &str) -> Result<Map<String, Value>, String> {
    let trimmed = proposal_content.trim();
    let value: Value = serde_json::from_str(trimmed)
        .map_err(|err| format!("proposal is not valid JSON: {err}"))?;
    match value {
        Value::Object(object) => Ok(object),
        other => Err(format!(
            "proposal root must be a JSON object, got {}",
            value_type_name(&other)
        )),
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn attributes(value: Value) -> Result<Attributes, DomainError> {
    let Value::Object(object) = value else {
        return Err(DomainError::InvariantViolated {
            reason: "validator details must be JSON objects",
        });
    };
    Attributes::new(object.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use choreo_core::value_objects::{OutputContract, OutputFieldRule};
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
        ] {
            assert!(report.passed());
        }
    }
}
