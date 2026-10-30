use made_core::value_objects::{
    AuditActorId, AuditActorKind, ProtocolContext, ProtocolId, ProtocolName, ProtocolVersion,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartProtocolInput {
    pub(crate) id: ProtocolId,
    pub(crate) definition_name: ProtocolName,
    pub(crate) definition_version: ProtocolVersion,
    pub(crate) context: ProtocolContext,
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

impl StartProtocolInput {
    #[must_use]
    pub fn new(
        id: ProtocolId,
        definition_name: ProtocolName,
        definition_version: ProtocolVersion,
        context: ProtocolContext,
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
    pub const fn id(&self) -> &ProtocolId {
        &self.id
    }
}
