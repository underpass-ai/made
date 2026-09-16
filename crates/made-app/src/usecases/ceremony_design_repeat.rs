use made_core::value_objects::{StepIteration, StepOutputField};
use serde_json::Value;

/// A bounded repeat-until policy for one stage.
///
/// Bounded is not decoration: the cap is what makes a stage that keeps
/// asking for one more turn end, and it is why the iteration count is
/// a value object rather than a number.
#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDesignRepeat {
    max_iterations: StepIteration,
    output_field: StepOutputField,
    equals: Value,
}

impl CeremonyDesignRepeat {
    #[must_use]
    pub fn new(
        max_iterations: StepIteration,
        output_field: StepOutputField,
        equals: Value,
    ) -> Self {
        Self {
            max_iterations,
            output_field,
            equals,
        }
    }

    #[must_use]
    pub const fn max_iterations(&self) -> StepIteration {
        self.max_iterations
    }

    #[must_use]
    pub const fn output_field(&self) -> &StepOutputField {
        &self.output_field
    }

    #[must_use]
    pub const fn equals(&self) -> &Value {
        &self.equals
    }
}
