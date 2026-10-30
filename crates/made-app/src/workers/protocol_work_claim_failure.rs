use made_core::error::DomainError;
use made_core::value_objects::ProtocolId;

/// Failure to inspect or claim one protocol while its sibling claims survive.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolWorkClaimFailure {
    protocol_id: ProtocolId,
    error: DomainError,
}

impl ProtocolWorkClaimFailure {
    #[must_use]
    pub const fn new(protocol_id: ProtocolId, error: DomainError) -> Self {
        Self { protocol_id, error }
    }

    #[must_use]
    pub const fn protocol_id(&self) -> &ProtocolId {
        &self.protocol_id
    }

    #[must_use]
    pub const fn error(&self) -> &DomainError {
        &self.error
    }
}
