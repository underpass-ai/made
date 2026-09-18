use made_core::value_objects::{AuditActorId, AuditActorKind, CeremonyId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeCeremonyInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) actor_id: AuditActorId,
    pub(crate) actor_kind: AuditActorKind,
}

impl ResumeCeremonyInput {
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
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
    pub fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }
}
