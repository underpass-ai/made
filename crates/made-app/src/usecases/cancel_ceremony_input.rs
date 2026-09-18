use made_core::value_objects::{AuditActorKind, CeremonyId, LifecycleReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelCeremonyInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) actor_id: String,
    pub(crate) actor_kind: AuditActorKind,
    pub(crate) reason: LifecycleReason,
}

impl CancelCeremonyInput {
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        actor_id: impl Into<String>,
        actor_kind: AuditActorKind,
        reason: LifecycleReason,
    ) -> Self {
        Self {
            instance_id,
            actor_id: actor_id.into(),
            actor_kind,
            reason,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }
}
