use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{HostActivationOutcome, HostActivationPort};
use made_core::value_objects::{
    HostActivationAdapterKind, HostActivationEnvelope, HostDeliveryRecord, IntegratorBinding,
};

/// The deployment that does not wake hosts.
///
/// It says `Unsupported` rather than pretending to have delivered, and
/// that word is what the rest of the system is built on: a delivery
/// nobody pushed stays queued and visible, discovery says so, and a
/// host that wants its work asks for it. A no-op that reported success
/// would leave an operator watching a queue that never drains and no
/// way to find out why.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoHostActivation;

impl NoHostActivation {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl HostActivationPort for NoHostActivation {
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

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{
        AttentionKind, AttentionReason, CeremonyId, CeremonyInterventionId, HostActivationMode,
        HostAddress, HostAgentIncarnation, HostDeliveryItem, HostDeliveryPolicy,
        HostDeliveryTarget, HostDestination, HostKind, IntegratorBindingId, IntegratorFence,
        IntegratorScope, RoleId,
    };
    use time::OffsetDateTime;

    #[tokio::test]
    async fn a_deployment_without_an_adapter_says_so() {
        let ceremony = CeremonyId::new("c-1").unwrap();
        let role = RoleId::new("INTEGRATOR").unwrap();
        let binding = IntegratorBinding::new(
            IntegratorBindingId::new("b-1").unwrap(),
            IntegratorScope::ceremony(ceremony.clone()),
            role,
            HostDestination::new(
                HostKind::new("generic").unwrap(),
                HostAddress::new("session-1").unwrap(),
                HostActivationMode::None,
            ),
            HostAgentIncarnation::new("run-1").unwrap(),
            OffsetDateTime::UNIX_EPOCH,
        );
        let item = HostDeliveryItem::intervention(
            ceremony.clone(),
            CeremonyInterventionId::new("i-1").unwrap(),
        );
        let record = HostDeliveryRecord::queued(
            item.clone(),
            HostDeliveryTarget::integrator_binding(binding.id().clone()),
            HostDeliveryPolicy::activation(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        let envelope = HostActivationEnvelope::new(
            record.id().clone(),
            binding.id().clone(),
            IntegratorFence::FIRST,
            item,
            ceremony,
            AttentionKind::InterventionRequested,
            AttentionReason::new("a supervisor asked a question").unwrap(),
            OffsetDateTime::UNIX_EPOCH,
        );

        let outcome = NoHostActivation::new()
            .activate(&binding, &record, &envelope)
            .await
            .unwrap();

        assert_eq!(outcome, HostActivationOutcome::Unsupported);
        assert_eq!(
            NoHostActivation::new().kind(),
            HostActivationAdapterKind::None
        );
    }
}
