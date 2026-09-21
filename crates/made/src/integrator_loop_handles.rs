use std::sync::Arc;

use made_app::services::attention::AttentionRecovery;
use made_app::usecases::integrator::{
    AcknowledgeIntegratorAttentionUseCase, AwaitIntegratorAttentionUseCase,
    BindCeremonyIntegratorUseCase, GetCeremonyIntegratorBindingUseCase,
    ListAttentionDeliveriesUseCase,
};

/// The integrator loop the service composed, held for the surfaces
/// that will carry it.
///
/// One handle rather than six fields on the application, because the
/// six are never composed separately: a use case reading a ledger no
/// projection fills would answer every question with an empty queue
/// and look healthy doing it.
#[derive(Clone)]
pub struct IntegratorLoopHandles {
    /// The walk of the feed the read paths share with the subscriber
    /// that wakes it after every append.
    pub recovery: Arc<AttentionRecovery>,
    pub bind: Arc<BindCeremonyIntegratorUseCase>,
    pub binding: Arc<GetCeremonyIntegratorBindingUseCase>,
    pub await_attention: Arc<AwaitIntegratorAttentionUseCase>,
    pub acknowledge: Arc<AcknowledgeIntegratorAttentionUseCase>,
    pub deliveries: Arc<ListAttentionDeliveriesUseCase>,
}

impl std::fmt::Debug for IntegratorLoopHandles {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("IntegratorLoopHandles").finish()
    }
}
