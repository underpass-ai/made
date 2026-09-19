use crate::entities::CeremonyAgentStatus;

/// One bounded, stable page of latest agent observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyAgentStatusPage {
    entries: Vec<CeremonyAgentStatus>,
    next_cursor: Option<String>,
}

impl CeremonyAgentStatusPage {
    #[must_use]
    pub fn new(entries: Vec<CeremonyAgentStatus>, next_cursor: Option<String>) -> Self {
        Self {
            entries,
            next_cursor,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[CeremonyAgentStatus] {
        &self.entries
    }
    #[must_use]
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }
}
