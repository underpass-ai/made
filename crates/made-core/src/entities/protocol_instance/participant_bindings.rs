use crate::entities::protocol_commands::BindParticipant;
use crate::entities::ProtocolCommand;

use super::{
    BTreeMap, DomainError, OffsetDateTime, ProtocolDefinition, ProtocolInstance,
    ProtocolParticipantBinding, RoleId, Specialty,
};

impl ProtocolInstance {
    /// Seat a role for this session.
    ///
    /// Rebinding is allowed and deliberate: a panel can become
    /// unavailable halfway through a working session, and a protocol
    /// that could not be re-seated would have to be abandoned and
    /// started again, losing everything already decided. What was
    /// seated before stays in the journal; the instance carries who is
    /// seated now, which is what the next step needs.
    pub fn bind_participant(
        &mut self,
        definition: &ProtocolDefinition,
        role_id: RoleId,
        specialty: Specialty,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let command = ProtocolCommand::BindParticipant(BindParticipant {
            role_id,
            specialty,
            now,
        });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }

    #[must_use]
    pub fn participant_bindings(&self) -> &BTreeMap<RoleId, ProtocolParticipantBinding> {
        &self.participant_bindings
    }

    /// The specialty a role's work should be put to, if this session
    /// seated one. `None` means the definition decides, as usual.
    #[must_use]
    pub fn bound_specialty(&self, role_id: &RoleId) -> Option<&Specialty> {
        self.participant_bindings
            .get(role_id)
            .map(ProtocolParticipantBinding::specialty)
    }
}
