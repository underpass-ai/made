use crate::error::DomainError;

pub(super) const MAX_BUNDLE_ID_LEN: usize = 128;
pub(super) const MAX_SCHEMA_VERSION_LEN: usize = 64;
pub(super) const MAX_ITEM_ID_LEN: usize = 128;
pub(super) const MAX_ITEM_KIND_LEN: usize = 64;
pub(super) const MAX_ITEM_TITLE_LEN: usize = 256;
pub(super) const MAX_REFERENCE_ID_LEN: usize = 128;
pub(super) const MAX_URI_LEN: usize = 2048;
pub(super) const MAX_ITEMS: usize = 256;
pub(super) const MAX_REFERENCES: usize = 512;

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

pub(super) fn validate_collection_len(
    field: &'static str,
    actual: usize,
    max: usize,
) -> Result<(), DomainError> {
    if actual > max {
        return Err(DomainError::OutOfRange {
            field,
            value: actual as f64,
            min: 0.0,
            max: max as f64,
        });
    }
    Ok(())
}

pub(super) fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}
