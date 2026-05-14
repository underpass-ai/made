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

/// A validator that runs a full JSON Schema document (embedded in
/// `OutputContract::json_schema`) against the proposal output.
///
/// Subsumes both the "JSON Schema" and the "bounded event proposal
/// shape" deliverables from Epic 4 of the backlog: bounded shapes
/// (`maxLength`, `maxItems`, `additionalProperties: false`, …) are
/// expressed as JSON Schema constraints rather than a bespoke
/// validator.
///
/// Implementation notes:
///
/// - empty schema body → no-op (passes). Tasks without a schema keep
///   using the field-rule validators above.
/// - schema body must be a valid JSON document; malformed JSON or an
///   unsupported schema fails the proposal with a clear summary.
/// - compilation happens per-call. Schemas are small in practice and
///   compilation cost is dwarfed by an LLM-generated proposal — if
///   profiling later says otherwise, the validator can grow a
///   schema-text cache without changing the port surface.
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonSchemaValidator;

impl JsonSchemaValidator {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidatorPort for JsonSchemaValidator {
    fn kind(&self) -> &'static str {
        "output-json-schema"
    }

    async fn validate(
        &self,
        proposal_content: &str,
        constraints: &TaskConstraints,
    ) -> Result<ValidatorReport, DomainError> {
        // Cap on the number of violations reported per failed
        // proposal. A pathological schema can produce hundreds of
        // sub-errors; the caller can re-validate locally for the full
        // list if needed.
        const MAX_REPORTED_VIOLATIONS: usize = 16;

        let Some(contract) = constraints.output_contract() else {
            return ValidatorReport::new(
                self.kind(),
                true,
                "no structured output contract configured",
                Attributes::empty(),
            );
        };
        let schema_body = contract.json_schema();
        if schema_body.is_empty() {
            return ValidatorReport::new(
                self.kind(),
                true,
                "contract has no embedded JSON Schema",
                Attributes::empty(),
            );
        }

        let schema_value: Value = match serde_json::from_str(schema_body) {
            Ok(v) => v,
            Err(err) => {
                return ValidatorReport::new(
                    self.kind(),
                    false,
                    format!("output_contract.json_schema is not valid JSON: {err}"),
                    attributes(json!({ "contract_id": contract.contract_id() }))?,
                );
            }
        };

        let compiled = match jsonschema::JSONSchema::compile(&schema_value) {
            Ok(c) => c,
            Err(err) => {
                return ValidatorReport::new(
                    self.kind(),
                    false,
                    format!("output_contract.json_schema is not a valid JSON Schema: {err}"),
                    attributes(json!({ "contract_id": contract.contract_id() }))?,
                );
            }
        };

        let instance: Value = match serde_json::from_str(proposal_content.trim()) {
            Ok(v) => v,
            Err(err) => {
                return ValidatorReport::new(
                    self.kind(),
                    false,
                    format!("proposal is not valid JSON: {err}"),
                    attributes(json!({ "contract_id": contract.contract_id() }))?,
                );
            }
        };

        // Collect violations into owned `Vec<Value>` immediately so
        // the iterator's borrow of `compiled` + `instance` ends before
        // we cross the function return.
        let violations: Vec<Value> = match compiled.validate(&instance) {
            Ok(()) => Vec::new(),
            Err(errors) => errors
                .take(MAX_REPORTED_VIOLATIONS)
                .map(|err| {
                    json!({
                        "instance_path": err.instance_path.to_string(),
                        "schema_path": err.schema_path.to_string(),
                        "reason": err.to_string(),
                    })
                })
                .collect(),
        };

        if violations.is_empty() {
            ValidatorReport::new(
                self.kind(),
                true,
                "output satisfies the embedded JSON Schema",
                Attributes::empty(),
            )
        } else {
            let summary = if let Some(first) = violations.first() {
                let path = first
                    .get("instance_path")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let reason = first.get("reason").and_then(Value::as_str).unwrap_or("");
                if path.is_empty() {
                    format!("schema violation: {reason}")
                } else {
                    format!("schema violation at `{path}`: {reason}")
                }
            } else {
                "schema validation failed".to_owned()
            };
            ValidatorReport::new(
                self.kind(),
                false,
                summary,
                attributes(json!({
                    "contract_id": contract.contract_id(),
                    "violations": violations,
                }))?,
            )
        }
    }
}

/// Defends downstream consumers against pathological structured
/// outputs: payloads that nest too deep, carry too many keys, or
/// blow up an array. A consumer that trusts the choreographer's
/// `OutputContract` chain should never have to second-guess these
/// limits; a `JsonSchema` can constrain shape but not raw size.
///
/// The validator runs **only** when the task has an `OutputContract`
/// — without one there is no structured-output expectation to bound.
/// In that case it short-circuits with a pass and a `not applicable`
/// note, mirroring the JsonSchemaValidator's posture.
///
/// Limits are conservative by default and tunable through the
/// `with_*` builder methods. Each one is an inclusive upper bound;
/// the validator counts at most one violation per dimension so a
/// huge payload that breaks several limits still produces a small,
/// human-readable report.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)] // every limit IS a "max_*" budget; renaming muddies intent
pub struct BoundedEventShapeValidator {
    /// Total UTF-8 byte length of the proposal content. Caps the
    /// envelope size a single deliberation can place on the bus.
    max_total_size_bytes: usize,
    /// Maximum nesting depth of objects + arrays combined.
    max_depth: usize,
    /// Maximum number of keys in any single object.
    max_object_keys: usize,
    /// Maximum number of elements in any single array.
    max_array_len: usize,
    /// Maximum UTF-8 byte length of any single string value.
    max_string_len: usize,
}

impl BoundedEventShapeValidator {
    /// Defaults chosen for use cases where the output is fed into a
    /// downstream event bus or audit log:
    ///
    /// - 256 KiB total — matches the `OutputContract.json_schema`
    ///   cap so the schema and the validated instance share a budget.
    /// - 32 levels of nesting — deeper than any realistic Report.
    /// - 256 object keys — Report fields plus generous extension room.
    /// - 1024 array elements — caps repeated-findings explosions.
    /// - 64 KiB per string — a single field cannot dominate the
    ///   envelope.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_total_size_bytes: 256 * 1024,
            max_depth: 32,
            max_object_keys: 256,
            max_array_len: 1024,
            max_string_len: 64 * 1024,
        }
    }

    #[must_use]
    pub const fn with_max_total_size_bytes(mut self, bytes: usize) -> Self {
        self.max_total_size_bytes = bytes;
        self
    }
    #[must_use]
    pub const fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }
    #[must_use]
    pub const fn with_max_object_keys(mut self, keys: usize) -> Self {
        self.max_object_keys = keys;
        self
    }
    #[must_use]
    pub const fn with_max_array_len(mut self, len: usize) -> Self {
        self.max_array_len = len;
        self
    }
    #[must_use]
    pub const fn with_max_string_len(mut self, len: usize) -> Self {
        self.max_string_len = len;
        self
    }
}

impl Default for BoundedEventShapeValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidatorPort for BoundedEventShapeValidator {
    fn kind(&self) -> &'static str {
        "output-bounded-event-shape"
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

        let trimmed = proposal_content.trim();
        let total_bytes = trimmed.len();
        if total_bytes > self.max_total_size_bytes {
            return ValidatorReport::new(
                self.kind(),
                false,
                format!(
                    "proposal is {total_bytes} bytes; limit is {} bytes",
                    self.max_total_size_bytes
                ),
                attributes(json!({
                    "contract_id": contract.contract_id(),
                    "violation": "max_total_size_bytes",
                    "limit": self.max_total_size_bytes,
                    "actual": total_bytes,
                }))?,
            );
        }

        let instance: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(err) => {
                return ValidatorReport::new(
                    self.kind(),
                    false,
                    format!("proposal is not valid JSON: {err}"),
                    attributes(json!({ "contract_id": contract.contract_id() }))?,
                );
            }
        };

        if let Some(violation) = self.walk(&instance, "$", 0) {
            return ValidatorReport::new(
                self.kind(),
                false,
                violation.summary(),
                attributes(json!({
                    "contract_id": contract.contract_id(),
                    "violation": violation.kind,
                    "path": violation.path,
                    "limit": violation.limit,
                    "actual": violation.actual,
                }))?,
            );
        }

        ValidatorReport::new(
            self.kind(),
            true,
            "proposal satisfies the event-shape budget",
            attributes(json!({ "contract_id": contract.contract_id() }))?,
        )
    }
}

impl BoundedEventShapeValidator {
    fn walk(&self, value: &Value, path: &str, depth: usize) -> Option<ShapeViolation> {
        if depth > self.max_depth {
            return Some(ShapeViolation {
                kind: "max_depth",
                path: path.to_owned(),
                limit: self.max_depth,
                actual: depth,
            });
        }
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => None,
            Value::String(s) => {
                if s.len() > self.max_string_len {
                    Some(ShapeViolation {
                        kind: "max_string_len",
                        path: path.to_owned(),
                        limit: self.max_string_len,
                        actual: s.len(),
                    })
                } else {
                    None
                }
            }
            Value::Array(items) => {
                if items.len() > self.max_array_len {
                    return Some(ShapeViolation {
                        kind: "max_array_len",
                        path: path.to_owned(),
                        limit: self.max_array_len,
                        actual: items.len(),
                    });
                }
                for (i, item) in items.iter().enumerate() {
                    if let Some(v) = self.walk(item, &format!("{path}[{i}]"), depth + 1) {
                        return Some(v);
                    }
                }
                None
            }
            Value::Object(map) => {
                if map.len() > self.max_object_keys {
                    return Some(ShapeViolation {
                        kind: "max_object_keys",
                        path: path.to_owned(),
                        limit: self.max_object_keys,
                        actual: map.len(),
                    });
                }
                for (key, child) in map {
                    if let Some(v) = self.walk(child, &format!("{path}.{key}"), depth + 1) {
                        return Some(v);
                    }
                }
                None
            }
        }
    }
}

#[derive(Debug)]
struct ShapeViolation {
    kind: &'static str,
    path: String,
    limit: usize,
    actual: usize,
}

impl ShapeViolation {
    fn summary(&self) -> String {
        format!(
            "{} at `{}`: {} exceeds limit {}",
            self.kind, self.path, self.actual, self.limit
        )
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
                choreo_core::value_objects::OutputFormat::JsonObject,
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
