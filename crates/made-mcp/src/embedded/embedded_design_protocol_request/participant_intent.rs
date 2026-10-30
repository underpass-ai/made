use made_app::usecases::{ProtocolDesignParticipant, ProtocolParticipantCapability};
use made_core::error::DomainError;
use made_core::value_objects::RoleId;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ParticipantIntent {
    role_id: String,
    #[serde(default)]
    capabilities: Vec<String>,
}

impl ParticipantIntent {
    pub(super) fn into_domain(self) -> Result<ProtocolDesignParticipant, DomainError> {
        Ok(ProtocolDesignParticipant::new(
            RoleId::new(self.role_id)?,
            self.capabilities
                .iter()
                .map(|capability| ProtocolParticipantCapability::parse(capability))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}
