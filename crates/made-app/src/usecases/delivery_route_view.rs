use made_core::value_objects::{
    DeliveryAttempt, HostDeliveryId, HostDeliveryLease, HostDeliveryObservation,
    HostDeliveryRecord, HostDeliveryStateKind, HostDeliveryTarget,
};

/// One route an intervention took towards a host, as the ledger holds it.
///
/// A view rather than the record itself, because the record carries a
/// policy and a bounded history that belong to an operator debugging
/// the ledger, not to somebody asking whether their question arrived.
/// What survives is the four things that answer that: where it went,
/// where it got to, how many tries it cost, and what the host said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryRouteView {
    delivery_id: HostDeliveryId,
    target: HostDeliveryTarget,
    state: HostDeliveryStateKind,
    attempt: DeliveryAttempt,
    lease: Option<HostDeliveryLease>,
    last_observation: Option<HostDeliveryObservation>,
}

impl DeliveryRouteView {
    #[must_use]
    pub fn of(record: &HostDeliveryRecord) -> Self {
        Self {
            delivery_id: record.id().clone(),
            target: record.target().clone(),
            state: record.state().kind(),
            attempt: record.attempt(),
            lease: record.state().lease().cloned(),
            last_observation: record.state().observation().cloned(),
        }
    }

    #[must_use]
    pub const fn delivery_id(&self) -> &HostDeliveryId {
        &self.delivery_id
    }

    #[must_use]
    pub const fn target(&self) -> &HostDeliveryTarget {
        &self.target
    }

    #[must_use]
    pub const fn state(&self) -> HostDeliveryStateKind {
        self.state
    }

    #[must_use]
    pub const fn attempt(&self) -> DeliveryAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn lease(&self) -> Option<&HostDeliveryLease> {
        self.lease.as_ref()
    }

    #[must_use]
    pub const fn last_observation(&self) -> Option<&HostDeliveryObservation> {
        self.last_observation.as_ref()
    }
}
