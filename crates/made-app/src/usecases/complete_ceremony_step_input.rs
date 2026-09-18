use made_core::value_objects::{AuditActorKind, CeremonyId, StepClaimFence, StepId, StepResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteCeremonyStepInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) step_id: StepId,
    pub(crate) claim_fence: StepClaimFence,
    pub(crate) result: StepResult,
    /// What kind of party finished it.
    ///
    /// Only the kind. Which seat runs this step is the definition's to
    /// say and the engine reads it there; what filled that seat is
    /// something only the caller can see.
    pub(crate) actor_kind: AuditActorKind,
}

impl CompleteCeremonyStepInput {
    /// Complete only the claim whose identity the caller captured before work.
    ///
    /// Omitting a fence is a compile error; wire adapters likewise reject omission.
    /// ```compile_fail
    /// use made_app::usecases::CompleteCeremonyStepInput;
    /// use made_core::value_objects::{AuditActorKind, CeremonyId, StepId, StepOutput, StepResult};
    /// let input = CompleteCeremonyStepInput::new(
    ///     CeremonyId::new("session").unwrap(), StepId::new("work").unwrap(),
    ///     StepResult::completed(StepOutput::empty()).unwrap(), AuditActorKind::Agent,
    /// );
    /// ```
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        step_id: StepId,
        result: StepResult,
        actor_kind: AuditActorKind,
        claim_fence: StepClaimFence,
    ) -> Self {
        Self {
            instance_id,
            step_id,
            claim_fence,
            result,
            actor_kind,
        }
    }

    #[must_use]
    pub fn claim_fence(&self) -> &StepClaimFence {
        &self.claim_fence
    }

    #[must_use]
    pub fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }

    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }

    #[must_use]
    pub const fn result(&self) -> &StepResult {
        &self.result
    }

    #[must_use]
    pub const fn actor_kind(&self) -> AuditActorKind {
        self.actor_kind
    }
}
