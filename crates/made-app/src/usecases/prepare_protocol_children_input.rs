use made_core::value_objects::{AuditActorKind, ProtocolId, StepClaimFence, StepId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareProtocolChildrenInput {
    pub instance_id: ProtocolId,
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub actor_kind: AuditActorKind,
}

impl PrepareProtocolChildrenInput {
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
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
