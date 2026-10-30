use made_core::value_objects::{ProtocolStep, StepExecutionRecord};

/// A declared protocol step paired with its execution record.
#[derive(Debug, Clone, Copy)]
pub struct ProtocolStepView<'a> {
    step: &'a ProtocolStep,
    record: &'a StepExecutionRecord,
}

impl<'a> ProtocolStepView<'a> {
    pub(super) const fn new(step: &'a ProtocolStep, record: &'a StepExecutionRecord) -> Self {
        Self { step, record }
    }

    #[must_use]
    pub fn step(&self) -> &'a ProtocolStep {
        self.step
    }

    #[must_use]
    pub fn record(&self) -> &'a StepExecutionRecord {
        self.record
    }

    /// Where this step's work happened, when it happened in the
    /// protocol this one succeeds.
    ///
    /// A caller reading a completed step that this session never ran
    /// needs to be told so here rather than having to notice it: the
    /// whole point of carrying evidence by reference is that it keeps
    /// saying where it came from.
    #[must_use]
    pub fn carried_from(&self) -> Option<&'a made_core::value_objects::SourceRecordRef> {
        self.record.carried_from()
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
