use serde::{Deserialize, Serialize};

use super::{ProtocolAgentActivityKind, ProtocolAgentStatus};

/// One globally ordered host assertion in a protocol's public activity feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolAgentActivity {
    sequence: u64,
    kind: ProtocolAgentActivityKind,
    status: Box<ProtocolAgentStatus>,
}

impl ProtocolAgentActivity {
    #[must_use]
    pub fn new(
        sequence: u64,
        kind: ProtocolAgentActivityKind,
        status: ProtocolAgentStatus,
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
    pub const fn kind(&self) -> ProtocolAgentActivityKind {
        self.kind
    }

    #[must_use]
    pub fn status(&self) -> &ProtocolAgentStatus {
        &self.status
    }
}
