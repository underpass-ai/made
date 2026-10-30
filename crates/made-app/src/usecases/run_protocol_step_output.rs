use made_core::entities::ProtocolInstance;
use made_core::value_objects::{StepAttempt, StepResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunProtocolStepOutput {
    instance: ProtocolInstance,
    attempt: StepAttempt,
    result: StepResult,
}

impl RunProtocolStepOutput {
    #[must_use]
    pub fn new(instance: ProtocolInstance, attempt: StepAttempt, result: StepResult) -> Self {
        Self {
            instance,
            attempt,
            result,
        }
    }

    #[must_use]
    pub fn instance(&self) -> &ProtocolInstance {
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
