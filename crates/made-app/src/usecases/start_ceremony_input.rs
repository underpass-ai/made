use made_core::value_objects::{
    AuditActorId, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartCeremonyInput {
    pub(crate) id: CeremonyId,
    pub(crate) definition_name: CeremonyName,
    pub(crate) definition_version: CeremonyVersion,
    pub(crate) context: CeremonyContext,
    /// Who is opening the session, in the caller's own terms.
    ///
    /// Not a role from the definition: at the start its roles are not
    /// filled yet, and whoever opens a session may be a participant,
    /// an operator, or a scheduler that never takes part.
    pub(crate) actor_id: AuditActorId,
    /// What kind of party that is.
    ///
    /// Carried, never worked out.
    pub(crate) actor_kind: AuditActorKind,
}

impl StartCeremonyInput {
    #[must_use]
    pub fn new(
        id: CeremonyId,
        definition_name: CeremonyName,
        definition_version: CeremonyVersion,
        context: CeremonyContext,
        actor_id: impl Into<String>,
        actor_kind: AuditActorKind,
    ) -> Self {
        Self {
            id,
            definition_name,
            definition_version,
            context,
            actor_id: AuditActorId::new(actor_id),
            actor_kind,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &CeremonyId {
        &self.id
    }
}
