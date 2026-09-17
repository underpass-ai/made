use made_core::error::DomainError;
use made_core::value_objects::{StateRepeatUntilCondition, StepId, StepOutputField};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StateRepeatUntilDocument {
    step: String,
    output_field: String,
    equals: Value,
}

impl StateRepeatUntilDocument {
    pub(super) fn into_domain(self) -> Result<StateRepeatUntilCondition, DomainError> {
        Ok(StateRepeatUntilCondition::new(
            StepId::new(self.step)?,
            StepOutputField::new(self.output_field)?,
            self.equals,
        ))
    }
}
