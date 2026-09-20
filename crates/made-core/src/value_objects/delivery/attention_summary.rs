use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{AttentionEventId, AttentionKind, AttentionReason};

/// What a host is told about the attention behind a delivery.
///
/// A summary rather than the projected event: the envelope crosses to a
/// process the engine does not control, so it carries what identifies
/// and explains the call, and nothing the engine reasoned with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttentionSummary {
    id: AttentionEventId,
    kind: AttentionKind,
    #[serde(with = "time::serde::rfc3339")]
    occurred_at: OffsetDateTime,
    reason: AttentionReason,
}

impl AttentionSummary {
    #[must_use]
    pub const fn new(
        id: AttentionEventId,
        kind: AttentionKind,
        occurred_at: OffsetDateTime,
        reason: AttentionReason,
    ) -> Self {
        Self {
            id,
            kind,
            occurred_at,
            reason,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &AttentionEventId {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> AttentionKind {
        self.kind
    }

    #[must_use]
    pub const fn occurred_at(&self) -> OffsetDateTime {
        self.occurred_at
    }

    #[must_use]
    pub const fn reason(&self) -> &AttentionReason {
        &self.reason
    }
}
