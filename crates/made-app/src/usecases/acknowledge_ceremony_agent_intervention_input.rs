use made_core::value_objects::{
    CeremonyId, CeremonyInterventionId, DeliveryRecipient, HostDeliveryLease,
    HostDeliveryObservation,
};

/// A host saying what it saw, with the ticket that proves it was offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcknowledgeCeremonyAgentInterventionInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) intervention_id: CeremonyInterventionId,
    pub(crate) lease: HostDeliveryLease,
    pub(crate) recipient: DeliveryRecipient,
    pub(crate) observation: HostDeliveryObservation,
}

impl AcknowledgeCeremonyAgentInterventionInput {
    #[must_use]
    pub const fn new(
        instance_id: CeremonyId,
        intervention_id: CeremonyInterventionId,
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
    pub const fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }

    #[must_use]
    pub const fn intervention_id(&self) -> &CeremonyInterventionId {
        &self.intervention_id
    }
}
