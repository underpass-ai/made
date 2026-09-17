use made_core::value_objects::{StepId, StepOutputField};
use serde_json::Value;

/// Design intent for an exact structured-output guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyDesignOutputFieldGuard {
    step_id: StepId,
    output_field: StepOutputField,
    expected: Value,
}

impl CeremonyDesignOutputFieldGuard {
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
}
