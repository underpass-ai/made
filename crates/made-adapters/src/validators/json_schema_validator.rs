use async_trait::async_trait;
use made_core::entities::{TaskConstraints, ValidatorReport};
use made_core::error::DomainError;
use made_core::ports::ValidatorPort;
use made_core::value_objects::Attributes;
use serde_json::{json, Value};

use super::json_validation::{attributes, strip_markdown_fences};

#[derive(Debug, Default, Clone, Copy)]
pub struct JsonSchemaValidator;

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

        let instance: Value = match serde_json::from_str(strip_markdown_fences(proposal_content)) {
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
