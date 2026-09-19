use serde::{Deserialize, Serialize};

/// How a connector establishes what happened after an ambiguous interruption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionRecoveryCapability {
    IdempotentByOperationId,
    QueryableByOperationId,
    ReconciliationRequired,
}

impl ExecutionRecoveryCapability {
    #[must_use]
    pub const fn supports_automatic_recovery(self) -> bool {
        !matches!(self, Self::ReconciliationRequired)
    }
}
