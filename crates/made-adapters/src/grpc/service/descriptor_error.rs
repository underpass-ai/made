use made_core::error::DomainError;

/// Errors surfaced while turning a [`pb::RegisterAgentRequest`] into a
/// domain [`AgentDescriptor`].
#[derive(Debug)]
pub(super) enum DescriptorError {
    MissingAgentSummary,
    Domain(DomainError),
}

impl From<DomainError> for DescriptorError {
    fn from(err: DomainError) -> Self {
        Self::Domain(err)
    }
}
