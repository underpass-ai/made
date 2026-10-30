use crate::value_objects::{HostDeliveryId, HostDeliveryRecord, HostDeliveryStateKind, ProtocolId};

use super::{HostDeliveryPageLimit, HostDeliveryTargetFilter};

/// One page of a question about what the ledger holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostDeliveryQuery {
    protocol_id: Option<ProtocolId>,
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
    pub fn in_protocol(mut self, protocol_id: ProtocolId) -> Self {
        self.protocol_id = Some(protocol_id);
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
    pub const fn protocol_id(&self) -> Option<&ProtocolId> {
        self.protocol_id.as_ref()
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
        if let Some(protocol_id) = &self.protocol_id {
            if record.item().protocol_id() != protocol_id {
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
