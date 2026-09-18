use serde::{Deserialize, Serialize};

use crate::value_objects::{AuditRecordHash, EventId};

use super::{CeremonyId, ChildGroupId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildCompletionRef {
    group_id: ChildGroupId,
    child_id: CeremonyId,
    terminal_event_id: EventId,
    terminal_record_hash: AuditRecordHash,
}

impl ChildCompletionRef {
    #[must_use]
    pub fn new(
        group_id: ChildGroupId,
        child_id: CeremonyId,
        terminal_event_id: EventId,
        terminal_record_hash: AuditRecordHash,
    ) -> Self {
        Self {
            group_id,
            child_id,
            terminal_event_id,
            terminal_record_hash,
        }
    }
    #[must_use]
    pub fn group_id(&self) -> &ChildGroupId {
        &self.group_id
    }
    #[must_use]
    pub fn child_id(&self) -> &CeremonyId {
        &self.child_id
    }
    #[must_use]
    pub fn terminal_event_id(&self) -> &EventId {
        &self.terminal_event_id
    }
    #[must_use]
    pub const fn terminal_record_hash(&self) -> AuditRecordHash {
        self.terminal_record_hash
    }
}
