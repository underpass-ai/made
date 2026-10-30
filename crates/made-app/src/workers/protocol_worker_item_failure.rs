use made_core::error::DomainError;
use made_core::value_objects::{ProtocolId, StepId};

/// Failure of one accepted worker item without discarding sibling outcomes.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolWorkerItemFailure {
    protocol_id: ProtocolId,
    step_id: StepId,
    error: DomainError,
}

impl ProtocolWorkerItemFailure {
    #[must_use]
    pub const fn new(protocol_id: ProtocolId, step_id: StepId, error: DomainError) -> Self {
        Self {
            protocol_id,
            step_id,
            error,
        }
    }

    #[must_use]
    pub const fn protocol_id(&self) -> &ProtocolId {
        &self.protocol_id
    }

    #[must_use]
    pub const fn step_id(&self) -> &StepId {
        &self.step_id
    }

    #[must_use]
    pub const fn error(&self) -> &DomainError {
        &self.error
    }
}
