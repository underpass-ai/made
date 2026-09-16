use made_core::entities::AuditRecord;
use made_core::value_objects::StreamVersion;

/// One page of a ceremony's stream, and where the reader now stands.
///
/// `next_version` is what the caller passes back as `from_version` to
/// continue: the version of the last record on this page, or the head
/// when the page is empty. `head_version` is how far the stream goes,
/// so a caller can tell "there is more" from "you are caught up"
/// without asking again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyEventPage {
    records: Vec<AuditRecord>,
    next_version: StreamVersion,
    head_version: StreamVersion,
}

impl CeremonyEventPage {
    #[must_use]
    pub fn new(
        records: Vec<AuditRecord>,
        next_version: StreamVersion,
        head_version: StreamVersion,
    ) -> Self {
        Self {
            records,
            next_version,
            head_version,
        }
    }

    /// The sealed records, in stream order.
    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }

    #[must_use]
    pub const fn next_version(&self) -> StreamVersion {
        self.next_version
    }

    #[must_use]
    pub const fn head_version(&self) -> StreamVersion {
        self.head_version
    }

    /// Whether the stream holds records this page did not reach.
    #[must_use]
    pub fn has_more(&self) -> bool {
        self.next_version < self.head_version
    }
}
