use crate::value_objects::{CeremonyId, HostDeliveryId, HostDeliveryRecord, HostDeliveryStateKind};

use super::{HostDeliveryPageLimit, HostDeliveryTargetFilter};

/// One page of a question about what the ledger holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostDeliveryQuery {
    ceremony_id: Option<CeremonyId>,
    state: Option<HostDeliveryStateKind>,
    target: HostDeliveryTargetFilter,
    limit: HostDeliveryPageLimit,
    cursor: Option<HostDeliveryId>,
}

impl HostDeliveryQuery {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn in_ceremony(mut self, ceremony_id: CeremonyId) -> Self {
        self.ceremony_id = Some(ceremony_id);
        self
    }

    #[must_use]
    pub fn in_state(mut self, state: HostDeliveryStateKind) -> Self {
        self.state = Some(state);
        self
    }

    #[must_use]
    pub fn to(mut self, target: HostDeliveryTargetFilter) -> Self {
        self.target = target;
        self
    }

    #[must_use]
    pub const fn of_size(mut self, limit: HostDeliveryPageLimit) -> Self {
        self.limit = limit;
        self
    }

    /// Continue after the last identifier of the previous page.
    #[must_use]
    pub fn after(mut self, cursor: HostDeliveryId) -> Self {
        self.cursor = Some(cursor);
        self
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> Option<&CeremonyId> {
        self.ceremony_id.as_ref()
    }

    #[must_use]
    pub const fn state(&self) -> Option<HostDeliveryStateKind> {
        self.state
    }

    #[must_use]
    pub const fn target(&self) -> &HostDeliveryTargetFilter {
        &self.target
    }

    #[must_use]
    pub const fn limit(&self) -> HostDeliveryPageLimit {
        self.limit
    }

    #[must_use]
    pub const fn cursor(&self) -> Option<&HostDeliveryId> {
        self.cursor.as_ref()
    }

    /// Whether a record belongs on this page, cursor aside.
    #[must_use]
    pub fn admits(&self, record: &HostDeliveryRecord) -> bool {
        if let Some(ceremony_id) = &self.ceremony_id {
            if record.item().ceremony_id() != ceremony_id {
                return false;
            }
        }
        if let Some(state) = self.state {
            if record.state().kind() != state {
                return false;
            }
        }
        self.target.admits(record.target())
    }
}
