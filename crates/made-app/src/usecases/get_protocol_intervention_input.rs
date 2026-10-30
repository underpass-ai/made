use made_core::value_objects::{ProtocolId, ProtocolInterventionId};

/// One intervention of one protocol, with every route it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetProtocolInterventionInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) intervention_id: ProtocolInterventionId,
}

impl GetProtocolInterventionInput {
    #[must_use]
    pub const fn new(instance_id: ProtocolId, intervention_id: ProtocolInterventionId) -> Self {
        Self {
            instance_id,
            intervention_id,
        }
    }

    #[must_use]
    pub const fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
