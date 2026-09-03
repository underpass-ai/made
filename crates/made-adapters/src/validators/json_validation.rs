use made_core::error::DomainError;
use made_core::value_objects::Attributes;
use serde_json::{Map, Value};

pub(super) fn parse_json_object(proposal_content: &str) -> Result<Map<String, Value>, String> {
    let trimmed = strip_markdown_fences(proposal_content);
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

/// Contracts govern content, not transport cosmetics: models routinely wrap
/// an otherwise-valid JSON payload in Markdown code fences (```json … ```)
/// even when told not to. When the whole trimmed payload is a single fenced
/// block, unwrap it before parsing; anything else (prose around the fence,
/// multiple blocks) is returned untouched and will fail JSON parsing as
/// before — this is deliberately narrow so the gate never "finds" JSON
/// buried inside surrounding text the model also emitted.
pub(super) fn strip_markdown_fences(content: &str) -> &str {
    let trimmed = content.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let Some(inner) = rest.strip_suffix("```") else {
        return trimmed;
    };
    // Drop the info string (e.g. `json`) on the opening fence line, if any.
    match inner.split_once('\n') {
        Some((info, body)) if !info.trim().contains(' ') => body.trim(),
        _ => inner.trim(),
    }
}

pub(super) fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub(super) fn attributes(value: Value) -> Result<Attributes, DomainError> {
    let Value::Object(object) = value else {
        return Err(DomainError::InvariantViolated {
            reason: "validator details must be JSON objects",
        });
    };
    Attributes::new(object.into_iter().collect())
}
