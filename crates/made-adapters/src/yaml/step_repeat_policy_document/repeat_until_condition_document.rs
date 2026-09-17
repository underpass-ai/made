use made_core::error::DomainError;
use made_core::value_objects::{RepeatUntilCondition, StepOutputField};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RepeatUntilConditionDocument {
    output_field: String,
    equals: Value,
}

impl RepeatUntilConditionDocument {
    pub(super) fn into_domain(self) -> Result<RepeatUntilCondition, DomainError> {
        Ok(RepeatUntilCondition::output_field_equals(
            StepOutputField::new(self.output_field)?,
            self.equals,
        ))
    }
}
