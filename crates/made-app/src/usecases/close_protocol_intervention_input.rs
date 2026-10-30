use made_core::value_objects::{AuditActorKind, ProtocolId, ProtocolInterventionId, RoleId};

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_field_names)] // three identifiers; a suffix-free name would say less
pub struct CloseProtocolInterventionInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) intervention_id: ProtocolInterventionId,
    pub(crate) role_id: RoleId,
    /// What kind of party fills that seat.
    ///
    /// Carried, never worked out. The engine sees a seat and cannot
    /// see what fills it.
    pub(crate) role_kind: AuditActorKind,
}

impl CloseProtocolInterventionInput {
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        intervention_id: ProtocolInterventionId,
        role_id: RoleId,
        role_kind: AuditActorKind,
    ) -> Self {
        Self {
            instance_id,
            intervention_id,
            role_id,
            role_kind,
        }
    }

    #[must_use]
    pub const fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
