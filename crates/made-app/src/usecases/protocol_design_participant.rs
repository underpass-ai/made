use made_core::value_objects::RoleId;

use super::protocol_participant_capability::ProtocolParticipantCapability;

/// One role the author wants seated at the working session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolDesignParticipant {
    role_id: RoleId,
    capabilities: Vec<ProtocolParticipantCapability>,
}

impl ProtocolDesignParticipant {
    #[must_use]
    pub fn new(
        role_id: RoleId,
        capabilities: impl IntoIterator<Item = ProtocolParticipantCapability>,
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
    pub fn capabilities(&self) -> &[ProtocolParticipantCapability] {
        &self.capabilities
    }
}
