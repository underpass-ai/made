use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{HostActivationOutcome, HostActivationPort};
use made_core::value_objects::{
    HostActivationAdapterKind, HostActivationEnvelope, HostDeliveryRecord, IntegratorBinding,
};

/// A deployment that wakes nobody, which is the default and is what a
/// binding with activation `none` is composed against.
pub(in crate::usecases) struct NoHostActivationFake;

#[async_trait]
impl HostActivationPort for NoHostActivationFake {
    async fn activate(
        &self,
        _binding: &IntegratorBinding,
        _record: &HostDeliveryRecord,
        _envelope: &HostActivationEnvelope,
    ) -> Result<HostActivationOutcome, DomainError> {
        Ok(HostActivationOutcome::Unsupported)
    }

    fn kind(&self) -> HostActivationAdapterKind {
        HostActivationAdapterKind::None
    }
}
