use serde::{Deserialize, Serialize};

use super::{CeremonyAgentActivityKind, CeremonyAgentStatus};

/// One globally ordered host assertion in a ceremony's public activity feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyAgentActivity {
    sequence: u64,
    kind: CeremonyAgentActivityKind,
    status: Box<CeremonyAgentStatus>,
}

impl CeremonyAgentActivity {
    #[must_use]
    pub fn new(
        sequence: u64,
        kind: CeremonyAgentActivityKind,
        status: CeremonyAgentStatus,
    ) -> Self {
        debug_assert!(sequence > 0, "activity sequences start at one");
        Self {
            sequence,
            kind,
            status: Box::new(status),
        }
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn kind(&self) -> CeremonyAgentActivityKind {
        self.kind
    }

    #[must_use]
    pub fn status(&self) -> &CeremonyAgentStatus {
        &self.status
    }
}
