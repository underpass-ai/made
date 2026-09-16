use made_core::value_objects::RoleId;

use super::ceremony_participant_capability::CeremonyParticipantCapability;

/// One role the author wants seated at the working session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyDesignParticipant {
    role_id: RoleId,
    capabilities: Vec<CeremonyParticipantCapability>,
}

impl CeremonyDesignParticipant {
    #[must_use]
    pub fn new(
        role_id: RoleId,
        capabilities: impl IntoIterator<Item = CeremonyParticipantCapability>,
    ) -> Self {
        Self {
            role_id,
            capabilities: capabilities.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    #[must_use]
    pub fn capabilities(&self) -> &[CeremonyParticipantCapability] {
        &self.capabilities
    }
}
