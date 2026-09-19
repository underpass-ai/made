use made_core::value_objects::{
    CeremonyId, DurationMs, ExecutionOperationId, LeaseOwnerId, StepClaimFence, StepId,
};

/// Exact worker operation presented to the authorization boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerAuthorizationTarget {
    EnforceDeadline {
        ceremony: CeremonyId,
    },
    Claim {
        ceremony: CeremonyId,
        step: StepId,
        operation: ExecutionOperationId,
        owner: LeaseOwnerId,
        lease_ttl: DurationMs,
    },
    Complete {
        ceremony: CeremonyId,
        step: StepId,
        operation: ExecutionOperationId,
        fence: StepClaimFence,
    },
    Renew {
        ceremony: CeremonyId,
        step: StepId,
        operation: ExecutionOperationId,
        fence: StepClaimFence,
    },
}

impl WorkerAuthorizationTarget {
    #[must_use]
    pub const fn ceremony(&self) -> &CeremonyId {
        match self {
            Self::EnforceDeadline { ceremony }
            | Self::Claim { ceremony, .. }
            | Self::Complete { ceremony, .. }
            | Self::Renew { ceremony, .. } => ceremony,
        }
    }
}
