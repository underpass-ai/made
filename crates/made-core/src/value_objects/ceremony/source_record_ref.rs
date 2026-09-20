use serde::{Deserialize, Serialize};

use super::{CeremonyId, StateVisit, StepAttempt, StepId};
use crate::value_objects::{AuditRecordHash, EventId};

/// One sealed record of another instance, addressed exactly.
///
/// [`CeremonyRecordRef`](super::CeremonyRecordRef) points at a record
/// inside the instance that holds it, which is the only thing a reason
/// or an intervention ever needs. A succession has to point across
/// instances: the evidence a successor carries was produced somewhere
/// else, under another definition, and the reference is worth nothing
/// if it cannot say where.
///
/// The record hash is part of the reference rather than a check made
/// beside it. A reference that named only the event would keep
/// pointing after the predecessor's chain was rewritten, and pointing
/// at work that is no longer what it was is exactly the failure this
/// exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRecordRef {
    ceremony_id: CeremonyId,
    step_id: StepId,
    event_id: EventId,
    record_hash: AuditRecordHash,
    state_visit: StateVisit,
    attempt: StepAttempt,
}

impl SourceRecordRef {
    #[must_use]
    pub const fn new(
        ceremony_id: CeremonyId,
        step_id: StepId,
        event_id: EventId,
        record_hash: AuditRecordHash,
        state_visit: StateVisit,
        attempt: StepAttempt,
    ) -> Self {
        Self {
            ceremony_id,
            step_id,
            event_id,
            record_hash,
            state_visit,
            attempt,
        }
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub const fn step_id(&self) -> &StepId {
        &self.step_id
    }

    #[must_use]
    pub const fn event_id(&self) -> &EventId {
        &self.event_id
    }

    #[must_use]
    pub const fn record_hash(&self) -> AuditRecordHash {
        self.record_hash
    }

    #[must_use]
    pub const fn state_visit(&self) -> StateVisit {
        self.state_visit
    }

    #[must_use]
    pub const fn attempt(&self) -> StepAttempt {
        self.attempt
    }
}
