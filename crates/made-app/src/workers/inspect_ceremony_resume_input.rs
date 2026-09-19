use made_core::value_objects::{CeremonyId, ExecutionRecoveryPageLimit, StepClaimFence};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectCeremonyResumeInput {
    pub ceremony_id: CeremonyId,
    pub after_claim: Option<StepClaimFence>,
    pub limit: ExecutionRecoveryPageLimit,
}
