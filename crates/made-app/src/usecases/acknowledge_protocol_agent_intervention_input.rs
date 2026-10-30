use made_core::value_objects::{
    DeliveryRecipient, HostDeliveryLease, HostDeliveryObservation, ProtocolId,
    ProtocolInterventionId,
};

/// A host saying what it saw, with the ticket that proves it was offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcknowledgeProtocolAgentInterventionInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) intervention_id: ProtocolInterventionId,
    pub(crate) lease: HostDeliveryLease,
    pub(crate) recipient: DeliveryRecipient,
    pub(crate) observation: HostDeliveryObservation,
}

impl AcknowledgeProtocolAgentInterventionInput {
    #[must_use]
    pub const fn new(
        instance_id: ProtocolId,
        intervention_id: ProtocolInterventionId,
        lease: HostDeliveryLease,
        recipient: DeliveryRecipient,
        observation: HostDeliveryObservation,
    ) -> Self {
        Self {
            instance_id,
            intervention_id,
            lease,
            recipient,
            observation,
        }
    }

    #[must_use]
    pub const fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }

    #[must_use]
    pub const fn intervention_id(&self) -> &ProtocolInterventionId {
        &self.intervention_id
    }
}
