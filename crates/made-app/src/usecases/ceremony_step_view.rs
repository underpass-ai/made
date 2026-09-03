use made_core::value_objects::{CeremonyStep, StepExecutionRecord};

/// A declared ceremony step paired with its execution record.
#[derive(Debug, Clone, Copy)]
pub struct CeremonyStepView<'a> {
    step: &'a CeremonyStep,
    record: &'a StepExecutionRecord,
}

impl<'a> CeremonyStepView<'a> {
    pub(super) const fn new(step: &'a CeremonyStep, record: &'a StepExecutionRecord) -> Self {
        Self { step, record }
    }

    #[must_use]
    pub fn step(&self) -> &'a CeremonyStep {
        self.step
    }

    #[must_use]
    pub fn record(&self) -> &'a StepExecutionRecord {
        self.record
    }

    #[must_use]
    pub fn repeat_condition_satisfied(&self) -> bool {
        self.step
            .repeat_policy()
            .is_none_or(|policy| policy.is_satisfied(self.record.output()))
    }

    #[must_use]
    pub fn repeat_limit_reached(&self) -> bool {
        self.step.repeat_policy().is_some_and(|policy| {
            self.record.status().is_success()
                && !policy.is_satisfied(self.record.output())
                && !policy.permits_another_iteration(self.record.iteration())
        })
    }
}
