use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{AuditActorId, AuditActorKind, ProtocolId, RoleId, Specialty};

/// Validated seating request for one protocol instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindProtocolParticipantsInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) seating: BTreeMap<RoleId, Specialty>,
    pub(crate) actor_id: AuditActorId,
    pub(crate) actor_kind: AuditActorKind,
}

impl BindProtocolParticipantsInput {
    pub fn new(
        instance_id: ProtocolId,
        seating: impl IntoIterator<Item = (RoleId, Specialty)>,
        actor_id: impl Into<String>,
        actor_kind: AuditActorKind,
    ) -> Result<Self, DomainError> {
        let seating = seating.into_iter().collect::<BTreeMap<_, _>>();
        if seating.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "protocol_participant_binding.seating",
            });
        }
        Ok(Self {
            instance_id,
            seating,
            actor_id: AuditActorId::new(actor_id),
            actor_kind,
        })
    }

    #[must_use]
    pub fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }

    #[must_use]
    pub fn seating(&self) -> &BTreeMap<RoleId, Specialty> {
        &self.seating
    }
}
