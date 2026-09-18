use made_core::entities::CeremonyInstance;
use made_core::value_objects::{StepAttempt, StepClaimFence, StreamVersion};

/// Accepted claim, captured before any later writer can replace it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartCeremonyStepOutput {
    instance: CeremonyInstance,
    attempt: StepAttempt,
    claim_fence: StepClaimFence,
    version: StreamVersion,
}
impl StartCeremonyStepOutput {
    pub(crate) fn new(
        instance: CeremonyInstance,
        attempt: StepAttempt,
        claim_fence: StepClaimFence,
        version: StreamVersion,
    ) -> Self {
        Self {
            instance,
            attempt,
            claim_fence,
            version,
        }
    }
    #[must_use]
    pub fn version(&self) -> StreamVersion {
        self.version
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
    pub fn claim_fence(&self) -> &StepClaimFence {
        &self.claim_fence
    }
}
