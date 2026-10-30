use made_core::value_objects::{EventId, ProtocolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptChildCompletionInput {
    pub child_id: ProtocolId,
    pub terminal_event_id: EventId,
}

impl AcceptChildCompletionInput {
    #[must_use]
    pub fn new(child_id: ProtocolId, terminal_event_id: EventId) -> Self {
        Self {
            child_id,
            terminal_event_id,
        }
    }
}
