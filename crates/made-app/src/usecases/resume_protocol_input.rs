use made_core::value_objects::{AuditActorId, AuditActorKind, ProtocolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeProtocolInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) actor_id: AuditActorId,
    pub(crate) actor_kind: AuditActorKind,
}

impl ResumeProtocolInput {
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        actor_id: impl Into<String>,
        actor_kind: AuditActorKind,
    ) -> Self {
        Self {
            instance_id,
            actor_id: AuditActorId::new(actor_id),
            actor_kind,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
