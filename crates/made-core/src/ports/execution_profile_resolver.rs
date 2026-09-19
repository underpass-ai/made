use crate::error::DomainError;
use crate::value_objects::ExecutionProfile;

/// Resolves a host-owned execution profile before a delegated claim.
///
/// MADE coordinates the claim and records the result; the host owns model
/// availability and capability negotiation behind this port.
pub trait ExecutionProfileResolverPort: Send + Sync {
    fn resolve(&self, profile: &ExecutionProfile) -> Result<ExecutionProfile, DomainError>;
}
