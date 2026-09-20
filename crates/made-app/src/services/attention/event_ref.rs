//! Where an attention event came from, so a host can go and look.

use made_core::value_objects::{AuditRecordHash, AuditSequence, EventId};
use serde::Serialize;

/// The sealed record an attention event was derived from.
///
/// Three identifiers rather than one: the event id is what a host
/// quotes back, the sequence is where it sits in its own stream, and
/// the hash is what proves the record has not been rewritten since
/// the projection read it. A host that acts on attention and later
/// wants to show its work needs all three.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventRef {
    event_id: EventId,
    sequence: AuditSequence,
    record_hash: AuditRecordHash,
}

impl EventRef {
    #[must_use]
    pub const fn new(
        event_id: EventId,
        sequence: AuditSequence,
        record_hash: AuditRecordHash,
    ) -> Self {
        Self {
            event_id,
            sequence,
            record_hash,
        }
    }

    #[must_use]
    pub const fn event_id(&self) -> &EventId {
        &self.event_id
    }

    #[must_use]
    pub const fn sequence(&self) -> AuditSequence {
        self.sequence
    }

    #[must_use]
    pub const fn record_hash(&self) -> AuditRecordHash {
        self.record_hash
    }
}
