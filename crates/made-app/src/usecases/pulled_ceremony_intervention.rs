use made_core::value_objects::HostDeliveryLease;

use super::ceremony_intervention_view::CeremonyInterventionView;

/// One question handed to an agent, with the lease it holds it under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulledCeremonyIntervention {
    lease: HostDeliveryLease,
    intervention: CeremonyInterventionView,
}

impl PulledCeremonyIntervention {
    #[must_use]
    pub const fn new(lease: HostDeliveryLease, intervention: CeremonyInterventionView) -> Self {
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
    pub const fn intervention(&self) -> &CeremonyInterventionView {
        &self.intervention
    }
}
