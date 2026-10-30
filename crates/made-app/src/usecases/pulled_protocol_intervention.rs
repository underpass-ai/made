use made_core::value_objects::HostDeliveryLease;

use super::protocol_intervention_view::ProtocolInterventionView;

/// One question handed to an agent, with the lease it holds it under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulledProtocolIntervention {
    lease: HostDeliveryLease,
    intervention: ProtocolInterventionView,
}

impl PulledProtocolIntervention {
    #[must_use]
    pub const fn new(lease: HostDeliveryLease, intervention: ProtocolInterventionView) -> Self {
        Self {
            lease,
            intervention,
        }
    }

    /// The lease, which is also the ticket an acknowledgement needs.
    #[must_use]
    pub const fn lease(&self) -> &HostDeliveryLease {
        &self.lease
    }

    #[must_use]
    pub const fn intervention(&self) -> &ProtocolInterventionView {
        &self.intervention
    }
}
