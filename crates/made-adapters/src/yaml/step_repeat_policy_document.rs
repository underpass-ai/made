use made_core::error::DomainError;
use made_core::value_objects::{StepIteration, StepRepeatPolicy};
use serde::Deserialize;

mod repeat_until_condition_document;

use repeat_until_condition_document::RepeatUntilConditionDocument;

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
