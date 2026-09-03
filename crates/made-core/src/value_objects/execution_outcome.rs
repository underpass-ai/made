use serde::{Deserialize, Serialize};

use super::{Attributes, DurationMs, ExecutionId, ExecutionStatus};

/// Final result returned by an execution adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    id: ExecutionId,
    status: ExecutionStatus,
    duration: DurationMs,
    output: Attributes,
}

impl ExecutionOutcome {
    #[must_use]
    pub const fn new(
        id: ExecutionId,
        status: ExecutionStatus,
        duration: DurationMs,
        output: Attributes,
    ) -> Self {
        Self {
            id,
            status,
            duration,
            output,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &ExecutionId {
        &self.id
    }

    #[must_use]
    pub const fn status(&self) -> ExecutionStatus {
        self.status
    }

    #[must_use]
    pub const fn duration(&self) -> DurationMs {
        self.duration
    }

    #[must_use]
    pub const fn output(&self) -> &Attributes {
        &self.output
    }
}
