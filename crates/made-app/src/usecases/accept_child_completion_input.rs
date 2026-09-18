use made_core::value_objects::{CeremonyId, EventId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptChildCompletionInput {
    pub child_id: CeremonyId,
    pub terminal_event_id: EventId,
}

impl AcceptChildCompletionInput {
    #[must_use]
    pub fn new(child_id: CeremonyId, terminal_event_id: EventId) -> Self {
        Self {
            child_id,
            terminal_event_id,
        }
    }
}
