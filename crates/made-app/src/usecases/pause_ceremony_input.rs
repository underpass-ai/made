use made_core::value_objects::{AuditActorId, AuditActorKind, CeremonyId, LifecycleReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PauseCeremonyInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) actor_id: AuditActorId,
    pub(crate) actor_kind: AuditActorKind,
    pub(crate) reason: LifecycleReason,
}

impl PauseCeremonyInput {
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
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
    pub fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }
}
