//! Waking the host a delivery is addressed to.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::value_objects::{
    HostActivationAdapterKind, HostActivationEnvelope, HostDeliveryRecord, IntegratorBinding,
};

use super::HostActivationOutcome;

/// Outbound contract for reaching a host that does not ask.
///
/// The engine hands over an envelope and learns whether it arrived.
/// What the host does next it does through the engine's own commands,
/// under their own authorization: an adapter here can wake a process
/// and can never grant it a permission.
///
/// Nothing in the envelope is ever executed. An adapter that runs
/// anything runs what its operator configured, with the envelope as
/// input, which is the difference between waking a host and letting a
/// payload choose a command.
#[async_trait]
pub trait HostActivationPort: Send + Sync {
    /// Hand one envelope to the bound host.
    async fn activate(
        &self,
        binding: &IntegratorBinding,
        record: &HostDeliveryRecord,
        envelope: &HostActivationEnvelope,
    ) -> Result<HostActivationOutcome, DomainError>;

    /// Which adapter this is, for discovery and for the receipt.
    fn kind(&self) -> HostActivationAdapterKind;
}
