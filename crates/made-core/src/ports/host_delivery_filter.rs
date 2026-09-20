use crate::value_objects::{CeremonyId, HostDeliveryItemKind, HostDeliveryRecord};

use super::HostDeliveryTargetFilter;

/// Which deliveries a host is offering to take.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostDeliveryFilter {
    ceremony_id: Option<CeremonyId>,
    target: HostDeliveryTargetFilter,
    item_kind: Option<HostDeliveryItemKind>,
}

impl HostDeliveryFilter {
    /// Deliveries to any of these destinations, in any ceremony.
    #[must_use]
    pub const fn to(target: HostDeliveryTargetFilter) -> Self {
        Self {
            ceremony_id: None,
            target,
            item_kind: None,
        }
    }

    /// Narrow to one ceremony.
    #[must_use]
    pub fn in_ceremony(mut self, ceremony_id: CeremonyId) -> Self {
        self.ceremony_id = Some(ceremony_id);
        self
    }

    /// Narrow to one kind of item.
    #[must_use]
    pub fn of_kind(mut self, item_kind: HostDeliveryItemKind) -> Self {
        self.item_kind = Some(item_kind);
        self
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> Option<&CeremonyId> {
        self.ceremony_id.as_ref()
    }

    #[must_use]
    pub const fn target(&self) -> &HostDeliveryTargetFilter {
        &self.target
    }

    #[must_use]
    pub const fn item_kind(&self) -> Option<HostDeliveryItemKind> {
        self.item_kind
    }

    /// Whether a record is one this filter asked for.
    ///
    /// Written once here so memory, SQLite and Postgres cannot disagree
    /// about what a filter means after each narrows the scan its own way.
    #[must_use]
    pub fn admits(&self, record: &HostDeliveryRecord) -> bool {
        if let Some(ceremony_id) = &self.ceremony_id {
            if record.item().ceremony_id() != ceremony_id {
                return false;
            }
        }
        if let Some(item_kind) = self.item_kind {
            if record.item().kind() != item_kind {
                return false;
            }
        }
        self.target.admits(record.target())
    }
}
