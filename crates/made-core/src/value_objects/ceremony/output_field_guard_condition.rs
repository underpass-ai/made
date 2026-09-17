use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{StepExecutionRecord, StepId, StepOutputField};

/// Exact structured-output predicate used by a ceremony guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputFieldGuardCondition {
    step_id: StepId,
    output_field: StepOutputField,
    expected: Value,
}

impl OutputFieldGuardCondition {
    #[must_use]
    pub fn new(step_id: StepId, output_field: StepOutputField, expected: Value) -> Self {
        Self {
            step_id,
            output_field,
            expected,
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
    pub fn expected(&self) -> &Value {
        &self.expected
    }

    #[must_use]
    pub fn is_satisfied(&self, record: &StepExecutionRecord) -> bool {
        record.status().is_success()
            && record.output().attributes().get(self.output_field.as_str()) == Some(&self.expected)
    }
}
