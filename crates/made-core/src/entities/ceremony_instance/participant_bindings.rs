use super::{
    BTreeMap, CeremonyDefinition, CeremonyInstance, CeremonyParticipantBinding, DomainError,
    OffsetDateTime, RoleId, Specialty,
};

impl CeremonyInstance {
    /// Seat a role for this session.
    ///
    /// Rebinding is allowed and deliberate: a panel can become
    /// unavailable halfway through a working session, and a ceremony
    /// that could not be re-seated would have to be abandoned and
    /// started again, losing everything already decided. What was
    /// seated before stays in the journal; the instance carries who is
    /// seated now, which is what the next step needs.
    pub fn bind_participant(
        &mut self,
        definition: &CeremonyDefinition,
        role_id: RoleId,
        specialty: Specialty,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot be re-seated",
        )?;
        // A seat that the ceremony never declared is not a seat.
        if definition.role(&role_id).is_none() {
            return Err(DomainError::NotFound {
                what: "ceremony_role",
            });
        }
        self.participant_bindings.insert(
            role_id.clone(),
            CeremonyParticipantBinding::record(role_id, specialty, now),
        );
        self.updated_at = now;
        Ok(())
    }

    #[must_use]
    pub fn participant_bindings(&self) -> &BTreeMap<RoleId, CeremonyParticipantBinding> {
        &self.participant_bindings
    }

    /// The specialty a role's work should be put to, if this session
    /// seated one. `None` means the definition decides, as usual.
    #[must_use]
    pub fn bound_specialty(&self, role_id: &RoleId) -> Option<&Specialty> {
        self.participant_bindings
            .get(role_id)
            .map(CeremonyParticipantBinding::specialty)
    }
}
