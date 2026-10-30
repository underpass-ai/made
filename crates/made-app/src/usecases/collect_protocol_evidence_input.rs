use made_core::value_objects::{
    AuditActorKind, ProtocolEvidenceSourceId, ProtocolId, ProtocolInterventionContent,
    ProtocolInterventionId, RoleId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectProtocolEvidenceInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) intervention_id: ProtocolInterventionId,
    pub(crate) role_id: RoleId,
    /// What kind of party fills that seat.
    ///
    /// Carried, never worked out. The engine sees a seat and cannot
    /// see what fills it.
    pub(crate) role_kind: AuditActorKind,
    pub(crate) source_id: ProtocolEvidenceSourceId,
    pub(crate) query: ProtocolInterventionContent,
}

impl CollectProtocolEvidenceInput {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        intervention_id: ProtocolInterventionId,
        role_id: RoleId,
        role_kind: AuditActorKind,
        source_id: ProtocolEvidenceSourceId,
        query: ProtocolInterventionContent,
    ) -> Self {
        Self {
            instance_id,
            intervention_id,
            role_id,
            role_kind,
            source_id,
            query,
        }
    }

    #[must_use]
    pub const fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
