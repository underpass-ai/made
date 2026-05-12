//! Structured output contract for a council invocation.
//!
//! This is intentionally generic and domain-agnostic. It does not know
//! what a "decision", "report", or "event" means; it only describes
//! the shape that a proposal must satisfy when a caller requires a
//! structured output instead of free-form text.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_CONTRACT_ID_LEN: usize = 128;
const MAX_FIELDS: usize = 128;
const MAX_FIELD_NAME_LEN: usize = 128;
const MAX_ALLOWED_VALUES_PER_FIELD: usize = 128;
const MAX_ALLOWED_VALUE_LEN: usize = 256;
/// Cap on the embedded JSON Schema body. 256 KiB is enough for an
/// elaborate Report-shape schema with nested objects and several
/// dozen enums; anything larger should live behind a `$ref` and be
/// fetched by the validator if/when remote schemas are supported.
const MAX_JSON_SCHEMA_LEN: usize = 256 * 1024;

/// Wire- and storage-stable structured output format selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OutputFormat {
    /// A single JSON object at the root.
    #[default]
    JsonObject,
}

impl OutputFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JsonObject => "json_object",
        }
    }
}

/// Validation rules for one named field in a structured output object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OutputFieldRule {
    required: bool,
    #[serde(default)]
    allowed_string_values: BTreeSet<String>,
}

impl OutputFieldRule {
    pub fn new(
        required: bool,
        allowed_string_values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, DomainError> {
        let values = allowed_string_values
            .into_iter()
            .map(|value| {
                let value = value.into();
                validate_text(
                    &value,
                    "output_contract.field.allowed_value",
                    MAX_ALLOWED_VALUE_LEN,
                )
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if values.len() > MAX_ALLOWED_VALUES_PER_FIELD {
            return Err(DomainError::OutOfRange {
                field: "output_contract.field.allowed_values",
                value: values.len() as f64,
                min: 0.0,
                max: MAX_ALLOWED_VALUES_PER_FIELD as f64,
            });
        }
        Ok(Self {
            required,
            allowed_string_values: values,
        })
    }

    #[must_use]
    pub const fn required(&self) -> bool {
        self.required
    }

    #[must_use]
    pub fn allowed_string_values(&self) -> &BTreeSet<String> {
        &self.allowed_string_values
    }
}

/// Typed structured-output contract attached to one invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputContract {
    contract_id: String,
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
        let contract_id = contract_id.into();
        let contract_id = validate_text(
            &contract_id,
            "output_contract.contract_id",
            MAX_CONTRACT_ID_LEN,
        )?;
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
        })
    }

    pub fn json_object(
        contract_id: impl Into<String>,
        fields: BTreeMap<String, OutputFieldRule>,
    ) -> Result<Self, DomainError> {
        Self::new(contract_id, OutputFormat::JsonObject, fields)
    }

    #[must_use]
    pub fn contract_id(&self) -> &str {
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
}

fn normalize_optional_schema(raw: &str) -> Result<String, DomainError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.len() > MAX_JSON_SCHEMA_LEN {
        return Err(DomainError::FieldTooLong {
            field: "output_contract.json_schema",
            actual: trimmed.len(),
            max: MAX_JSON_SCHEMA_LEN,
        });
    }
    Ok(trimmed.to_owned())
}

fn validate_text(value: &str, field: &'static str, max_len: usize) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    if trimmed.len() > max_len {
        return Err(DomainError::FieldTooLong {
            field,
            actual: trimmed.len(),
            max: max_len,
        });
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

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
