use crate::error::DomainError;

pub(super) const MAX_FIELDS: usize = 128;
pub(super) const MAX_FIELD_NAME_LEN: usize = 128;
pub(super) const MAX_ALLOWED_VALUES_PER_FIELD: usize = 128;
pub(super) const MAX_ALLOWED_VALUE_LEN: usize = 256;
pub(super) const MAX_JSON_SCHEMA_LEN: usize = 256 * 1024;
pub(super) const MAX_ALLOWED_EVIDENCE_REFS: usize = 1024;
pub(super) const MAX_EVIDENCE_BODY_LEN: usize = 16 * 1024;

pub(super) fn normalize_optional_schema(raw: &str) -> Result<String, DomainError> {
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

pub(super) fn validate_text(
    value: &str,
    field: &'static str,
    max_len: usize,
) -> Result<String, DomainError> {
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
