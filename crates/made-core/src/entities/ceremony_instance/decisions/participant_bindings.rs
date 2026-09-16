use crate::entities::ceremony_commands::BindParticipant;
use crate::entities::ceremony_events::ParticipantsBound;
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::CeremonyParticipantBinding;

impl CeremonyInstance {
    /// One command seats one role, so the event carries one binding.
    /// Seating a role again is allowed and deliberate: the fold
    /// replaces the earlier binding, and the journal keeps it.
    pub(super) fn decide_bind_participant(
        &self,
        command: &BindParticipant,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot be re-seated",
        )?;
        // A seat that the ceremony never declared is not a seat.
        if definition.role(&command.role_id).is_none() {
            return Err(DomainError::NotFound {
                what: "ceremony_role",
            });
        }
        Ok(vec![CeremonyEvent::ParticipantsBound(ParticipantsBound {
            bindings: vec![CeremonyParticipantBinding::record(
                command.role_id.clone(),
                command.specialty.clone(),
                command.now,
            )],
        })])
    }
}
