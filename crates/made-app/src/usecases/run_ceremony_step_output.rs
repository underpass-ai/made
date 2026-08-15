use made_core::entities::CeremonyInstance;
use made_core::value_objects::{StepAttempt, StepResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCeremonyStepOutput {
    instance: CeremonyInstance,
    attempt: StepAttempt,
    result: StepResult,
}

impl RunCeremonyStepOutput {
    #[must_use]
    pub fn new(instance: CeremonyInstance, attempt: StepAttempt, result: StepResult) -> Self {
        Self {
            instance,
            attempt,
            result,
        }
    }

    #[must_use]
    pub fn instance(&self) -> &CeremonyInstance {
        &self.instance
    }

    #[must_use]
    pub fn attempt(&self) -> StepAttempt {
        self.attempt
    }

    #[must_use]
    pub fn result(&self) -> &StepResult {
        &self.result
    }
}
