use crate::entities::ProtocolAgentStatus;

/// One bounded, stable page of latest agent observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolAgentStatusPage {
    entries: Vec<ProtocolAgentStatus>,
    next_cursor: Option<String>,
}

impl ProtocolAgentStatusPage {
    #[must_use]
    pub fn new(entries: Vec<ProtocolAgentStatus>, next_cursor: Option<String>) -> Self {
        Self {
            entries,
            next_cursor,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[ProtocolAgentStatus] {
        &self.entries
    }
    #[must_use]
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }
}
