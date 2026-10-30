use made_core::value_objects::{
    DurationMs, ExecutionOperationId, LeaseOwnerId, ProtocolId, StepClaimFence, StepId,
};

/// Exact worker operation presented to the authorization boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerAuthorizationTarget {
    EnforceDeadline {
        protocol: ProtocolId,
    },
    Claim {
        protocol: ProtocolId,
        step: StepId,
        operation: ExecutionOperationId,
        owner: LeaseOwnerId,
        lease_ttl: DurationMs,
    },
    Complete {
        protocol: ProtocolId,
        step: StepId,
        operation: ExecutionOperationId,
        fence: StepClaimFence,
    },
    Renew {
        protocol: ProtocolId,
        step: StepId,
        operation: ExecutionOperationId,
        fence: StepClaimFence,
    },
}

impl WorkerAuthorizationTarget {
    #[must_use]
    pub const fn protocol(&self) -> &ProtocolId {
        match self {
            Self::EnforceDeadline { protocol }
            | Self::Claim { protocol, .. }
            | Self::Complete { protocol, .. }
            | Self::Renew { protocol, .. } => protocol,
        }
    }
}
