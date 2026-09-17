use made_core::value_objects::{StepId, StepOutputField};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDesignGroupRepeatUntil {
    step_id: StepId,
    output_field: StepOutputField,
    equals: Value,
}

impl CeremonyDesignGroupRepeatUntil {
    #[must_use]
    pub fn new(step_id: StepId, output_field: StepOutputField, equals: Value) -> Self {
        Self {
            step_id,
            output_field,
            equals,
        }
    }
    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    #[must_use]
    pub fn output_field(&self) -> &StepOutputField {
        &self.output_field
    }
    #[must_use]
    pub fn equals(&self) -> &Value {
        &self.equals
    }
}
