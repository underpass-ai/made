use made_core::value_objects::{AuditActorId, AuditActorKind, LifecycleReason, ProtocolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PauseProtocolInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) actor_id: AuditActorId,
    pub(crate) actor_kind: AuditActorKind,
    pub(crate) reason: LifecycleReason,
}

impl PauseProtocolInput {
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        actor_id: impl Into<String>,
        actor_kind: AuditActorKind,
        reason: LifecycleReason,
    ) -> Self {
        Self {
            instance_id,
            actor_id: AuditActorId::new(actor_id),
            actor_kind,
            reason,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
