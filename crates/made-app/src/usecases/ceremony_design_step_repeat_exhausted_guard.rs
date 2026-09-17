use made_core::value_objects::StepId;

/// Design intent for routing a step that consumed its repeat cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyDesignStepRepeatExhaustedGuard {
    step_id: StepId,
}

impl CeremonyDesignStepRepeatExhaustedGuard {
    #[must_use]
    pub fn new(step_id: StepId) -> Self {
        Self { step_id }
    }

    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
}
