use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{StepExecutionRecord, StepId, StepOutputField};

/// The exact output predicate that stops repetition of a whole state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateRepeatUntilCondition {
    step_id: StepId,
    output_field: StepOutputField,
    equals: Value,
}

impl StateRepeatUntilCondition {
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

    #[must_use]
    pub fn is_satisfied(&self, record: &StepExecutionRecord) -> bool {
        record.status().is_success()
            && record.output().attributes().get(self.output_field.as_str()) == Some(&self.equals)
    }
}
