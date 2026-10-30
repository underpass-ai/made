use made_core::entities::ProtocolInstance;
use made_core::value_objects::{StepAttempt, StepClaimFence, StreamVersion};

/// Accepted claim, captured before any later writer can replace it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartProtocolStepOutput {
    instance: ProtocolInstance,
    attempt: StepAttempt,
    claim_fence: StepClaimFence,
    version: StreamVersion,
}
impl StartProtocolStepOutput {
    pub(crate) fn new(
        instance: ProtocolInstance,
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
    pub fn instance(&self) -> &ProtocolInstance {
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
