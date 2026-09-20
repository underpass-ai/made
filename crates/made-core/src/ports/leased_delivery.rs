use crate::value_objects::{HostDeliveryLease, HostDeliveryRecord};

/// One delivery, handed out with the lease that holds it.
///
/// The two travel together because a caller needs both and reading the
/// record back to find its lease would race the expiry that just
/// handed the same delivery to somebody else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeasedDelivery {
    lease: HostDeliveryLease,
    record: HostDeliveryRecord,
}

impl LeasedDelivery {
    #[must_use]
    pub const fn new(lease: HostDeliveryLease, record: HostDeliveryRecord) -> Self {
        Self { lease, record }
    }

    #[must_use]
    pub const fn lease(&self) -> &HostDeliveryLease {
        &self.lease
    }

    #[must_use]
    pub const fn record(&self) -> &HostDeliveryRecord {
        &self.record
    }

    #[must_use]
    pub fn into_parts(self) -> (HostDeliveryLease, HostDeliveryRecord) {
        (self.lease, self.record)
    }
}
