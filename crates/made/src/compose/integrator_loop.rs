//! The integrator loop, wired for the deployable service.
//!
//! Two calls rather than one because the loop is woken from two sides.
//! The projection is built first: the subscriber that wakes it after
//! every append is installed in the fanout, which exists before the
//! stream the five use cases hang off. Both sides then share one walk
//! of the feed and one cursor per binding.

use std::sync::Arc;

use made_app::services::attention::{
    AttentionAudienceResolver, AttentionProjector, AttentionRecovery,
};
use made_app::services::SessionStream;
use made_app::usecases::integrator::{
    AcknowledgeIntegratorAttentionUseCase, AwaitIntegratorAttentionUseCase,
    BindCeremonyIntegratorUseCase, GetCeremonyIntegratorBindingUseCase,
    ListAttentionDeliveriesUseCase,
};
use made_app::usecases::ResolveCeremonyDefinitionUseCase;
use made_core::ports::{
    CeremonyEventCursorPort, CeremonyEventStorePort, CeremonyProgressNotifierPort, ClockPort,
};

use crate::{AgenticSystemHandles, HostDeliveryHandles, IntegratorLoopHandles};

/// The projection one bound integrator's round runs through.
///
/// Activation comes from the delivery handles, so a service configured
/// to wake nobody projects exactly as one that can: the difference
/// shows up in the ledger as an offer that was never handed over,
/// which is what an operator needs to be able to read.
pub(super) fn recovery(
    events: Arc<dyn CeremonyEventStorePort>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
    host_delivery: &HostDeliveryHandles,
    systems: &AgenticSystemHandles,
    clock: Arc<dyn ClockPort>,
) -> Arc<AttentionRecovery> {
    Arc::new(AttentionRecovery::new(
        host_delivery.bindings.clone(),
        Arc::new(AttentionAudienceResolver::new(
            systems.executions.clone(),
            systems.repository.clone(),
        )),
        Arc::new(AttentionProjector::new(
            events,
            cursors,
            host_delivery.ledger.clone(),
            host_delivery.activation.clone(),
            clock,
        )),
    ))
}

/// The five verbs, over the projection already built.
pub(super) fn wire(
    recovery: Arc<AttentionRecovery>,
    host_delivery: &HostDeliveryHandles,
    events: Arc<dyn CeremonyEventStorePort>,
    stream: Arc<SessionStream>,
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    notifier: Arc<dyn CeremonyProgressNotifierPort>,
    clock: Arc<dyn ClockPort>,
) -> IntegratorLoopHandles {
    let bindings = host_delivery.bindings.clone();
    let ledger = host_delivery.ledger.clone();
    IntegratorLoopHandles {
        bind: Arc::new(BindCeremonyIntegratorUseCase::new(
            bindings.clone(),
            ledger.clone(),
            clock.clone(),
        )),
        binding: Arc::new(GetCeremonyIntegratorBindingUseCase::new(bindings.clone())),
        await_attention: Arc::new(
            AwaitIntegratorAttentionUseCase::new(
                bindings.clone(),
                ledger.clone(),
                events,
                stream,
                definitions,
                notifier,
                clock.clone(),
            )
            .with_recovery(recovery.clone()),
        ),
        acknowledge: Arc::new(AcknowledgeIntegratorAttentionUseCase::new(
            bindings,
            ledger.clone(),
            clock,
        )),
        deliveries: Arc::new(
            ListAttentionDeliveriesUseCase::new(ledger).with_recovery(recovery.clone()),
        ),
        recovery,
    }
}
