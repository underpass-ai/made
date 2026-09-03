use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{AuditActorKind, CeremonyId, RoleId, Specialty};

/// Validated seating request for one ceremony instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindCeremonyParticipantsInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) seating: BTreeMap<RoleId, Specialty>,
    pub(crate) actor_id: String,
    pub(crate) actor_kind: AuditActorKind,
}

impl BindCeremonyParticipantsInput {
    pub fn new(
        instance_id: CeremonyId,
        seating: impl IntoIterator<Item = (RoleId, Specialty)>,
        actor_id: impl Into<String>,
        actor_kind: AuditActorKind,
    ) -> Result<Self, DomainError> {
        let seating = seating.into_iter().collect::<BTreeMap<_, _>>();
        if seating.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "ceremony_participant_binding.seating",
            });
        }
        Ok(Self {
            instance_id,
            seating,
            actor_id: actor_id.into(),
            actor_kind,
        })
    }

    #[must_use]
    pub fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }

    #[must_use]
    pub fn seating(&self) -> &BTreeMap<RoleId, Specialty> {
        &self.seating
    }
}
