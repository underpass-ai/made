use made_core::error::DomainError;
use made_core::value_objects::{StateIteration, StateRepeatPolicy};
use serde::Deserialize;

use super::state_repeat_until_document::StateRepeatUntilDocument;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StateRepeatPolicyDocument {
    max_iterations: u32,
    until: StateRepeatUntilDocument,
}

impl StateRepeatPolicyDocument {
    pub(super) fn into_domain(self) -> Result<StateRepeatPolicy, DomainError> {
        Ok(StateRepeatPolicy::new(
            StateIteration::new(self.max_iterations)?,
            self.until.into_domain()?,
        ))
    }
}
