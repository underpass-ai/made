use made_core::error::DomainError;
use made_core::value_objects::{
    RepeatUntilCondition, StepIteration, StepOutputField, StepRepeatPolicy,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StepRepeatPolicyDocument {
    max_iterations: u32,
    until: RepeatUntilConditionDocument,
}

impl StepRepeatPolicyDocument {
    pub(super) fn into_domain(self) -> Result<StepRepeatPolicy, DomainError> {
        Ok(StepRepeatPolicy::new(
            self.until.into_domain()?,
            StepIteration::new(self.max_iterations)?,
        ))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepeatUntilConditionDocument {
    output_field: String,
    equals: Value,
}

impl RepeatUntilConditionDocument {
    fn into_domain(self) -> Result<RepeatUntilCondition, DomainError> {
        Ok(RepeatUntilCondition::output_field_equals(
            StepOutputField::new(self.output_field)?,
            self.equals,
        ))
    }
}
