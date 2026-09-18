use made_core::value_objects::{AuditActorKind, CeremonyId, StepClaimFence, StepId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareCeremonyChildrenInput {
    pub instance_id: CeremonyId,
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub actor_kind: AuditActorKind,
}

impl PrepareCeremonyChildrenInput {
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        step_id: StepId,
        claim_fence: StepClaimFence,
        actor_kind: AuditActorKind,
    ) -> Self {
        Self {
            instance_id,
            step_id,
            claim_fence,
            actor_kind,
        }
    }
}
