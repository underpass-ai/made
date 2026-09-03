use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::output_contract_validation::{
    validate_text, MAX_ALLOWED_VALUES_PER_FIELD, MAX_ALLOWED_VALUE_LEN,
};

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
                validate_text(
                    &value.into(),
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
