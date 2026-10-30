use crate::entities::protocol_commands::BindParticipant;
use crate::entities::protocol_events::ParticipantsBound;
use crate::entities::{ProtocolDefinition, ProtocolEvent, ProtocolInstance};
use crate::error::DomainError;
use crate::value_objects::ProtocolParticipantBinding;

impl ProtocolInstance {
    /// One command seats one role, so the event carries one binding.
    /// Seating a role again is allowed and deliberate: the fold
    /// replaces the earlier binding, and the journal keeps it.
    pub(super) fn decide_bind_participant(
        &self,
        command: &BindParticipant,
        definition: &ProtocolDefinition,
    ) -> Result<Vec<ProtocolEvent>, DomainError> {
        self.require_active(
            definition,
            "terminal protocol instances cannot be re-seated",
        )?;
        // A seat that the protocol never declared is not a seat.
        if definition.role(&command.role_id).is_none() {
            return Err(DomainError::NotFound {
                what: "protocol_role",
            });
        }
        Ok(vec![ProtocolEvent::ParticipantsBound(ParticipantsBound {
            bindings: vec![ProtocolParticipantBinding::record(
                command.role_id.clone(),
                command.specialty.clone(),
                command.now,
            )],
        })])
    }
}
