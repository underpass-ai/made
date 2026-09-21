use std::sync::Arc;

use made_app::usecases::integrator::{
    AcknowledgeIntegratorAttentionUseCase, AwaitIntegratorAttentionUseCase,
    BindCeremonyIntegratorUseCase, GetCeremonyIntegratorBindingUseCase,
    ListAttentionDeliveriesUseCase,
};

/// The five verbs of the integrator loop, as the service holds them.
///
/// One field on the service rather than five, because they are never
/// composed separately: the composition root builds them over one
/// projection, and a service holding four of them would answer the
/// fifth question with an empty queue and look healthy doing it.
#[derive(Clone)]
pub struct IntegratorLoopOperations {
    bind: Arc<BindCeremonyIntegratorUseCase>,
    binding: Arc<GetCeremonyIntegratorBindingUseCase>,
    await_attention: Arc<AwaitIntegratorAttentionUseCase>,
    acknowledge: Arc<AcknowledgeIntegratorAttentionUseCase>,
    deliveries: Arc<ListAttentionDeliveriesUseCase>,
}

impl IntegratorLoopOperations {
    #[must_use]
    pub const fn new(
        bind: Arc<BindCeremonyIntegratorUseCase>,
        binding: Arc<GetCeremonyIntegratorBindingUseCase>,
        await_attention: Arc<AwaitIntegratorAttentionUseCase>,
        acknowledge: Arc<AcknowledgeIntegratorAttentionUseCase>,
        deliveries: Arc<ListAttentionDeliveriesUseCase>,
    ) -> Self {
        Self {
            bind,
            binding,
            await_attention,
            acknowledge,
            deliveries,
        }
    }

    pub(super) const fn bind(&self) -> &Arc<BindCeremonyIntegratorUseCase> {
        &self.bind
    }

    pub(super) const fn binding(&self) -> &Arc<GetCeremonyIntegratorBindingUseCase> {
        &self.binding
    }

    pub(super) const fn await_attention(&self) -> &Arc<AwaitIntegratorAttentionUseCase> {
        &self.await_attention
    }

    pub(super) const fn acknowledge(&self) -> &Arc<AcknowledgeIntegratorAttentionUseCase> {
        &self.acknowledge
    }

    pub(super) const fn deliveries(&self) -> &Arc<ListAttentionDeliveriesUseCase> {
        &self.deliveries
    }
}

impl std::fmt::Debug for IntegratorLoopOperations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("IntegratorLoopOperations").finish()
    }
}
