use crate::value_objects::{HostDeliveryId, HostDeliveryRecord};

/// One page of the ledger, in identifier order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostDeliveryPage {
    records: Vec<HostDeliveryRecord>,
    next_cursor: Option<HostDeliveryId>,
}

impl HostDeliveryPage {
    #[must_use]
    pub const fn new(
        records: Vec<HostDeliveryRecord>,
        next_cursor: Option<HostDeliveryId>,
    ) -> Self {
        Self {
            records,
            next_cursor,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[HostDeliveryRecord] {
        &self.records
    }

    #[must_use]
    pub fn into_records(self) -> Vec<HostDeliveryRecord> {
        self.records
    }

    /// Where to continue, when there is more.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<&HostDeliveryId> {
        self.next_cursor.as_ref()
    }
}
