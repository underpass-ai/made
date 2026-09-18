use made_core::value_objects::{AuditActorKind, CeremonyId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeCeremonyInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) actor_id: String,
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
            actor_id: actor_id.into(),
            actor_kind,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }
}
