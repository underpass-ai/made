//! The integrator loop, composed for the embedded engine.

use std::fmt;
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

use crate::agentic_system_ports::AgenticSystemPorts;
use crate::host_delivery_ports::HostDeliveryPorts;

/// The five verbs of the loop and the projection they read behind.
///
/// One handle rather than six fields on the engine, because they are
/// never composed separately: a use case reading a ledger no projector
/// fills would answer every question with an empty queue and look
/// healthy doing it.
#[derive(Clone)]
pub struct IntegratorLoopPorts {
    recovery: Arc<AttentionRecovery>,
    bind: Arc<BindCeremonyIntegratorUseCase>,
    binding: Arc<GetCeremonyIntegratorBindingUseCase>,
    await_attention: Arc<AwaitIntegratorAttentionUseCase>,
    acknowledge: Arc<AcknowledgeIntegratorAttentionUseCase>,
    deliveries: Arc<ListAttentionDeliveriesUseCase>,
}

impl IntegratorLoopPorts {
    /// Compose the five verbs over a projection that already exists.
    ///
    /// The projection is built first and separately because the append
    /// subscriber needs it before there is a stream to hang use cases
    /// off: one walk of the feed, woken from two sides.
    pub(crate) fn wire(
        recovery: Arc<AttentionRecovery>,
        host_delivery: &HostDeliveryPorts,
        events: Arc<dyn CeremonyEventStorePort>,
        stream: Arc<SessionStream>,
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        notifier: Arc<dyn CeremonyProgressNotifierPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        let bindings = host_delivery.bindings().clone();
        let ledger = host_delivery.ledger().clone();
        Self {
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

    /// Who drives which scope, and taking that over.
    #[must_use]
    pub fn bind(&self) -> &Arc<BindCeremonyIntegratorUseCase> {
        &self.bind
    }

    /// Who is driving a scope now.
    #[must_use]
    pub fn binding(&self) -> &Arc<GetCeremonyIntegratorBindingUseCase> {
        &self.binding
    }

    /// What a bound host is owed.
    #[must_use]
    pub fn await_attention(&self) -> &Arc<AwaitIntegratorAttentionUseCase> {
        &self.await_attention
    }

    /// What the host says it did about it.
    #[must_use]
    pub fn acknowledge(&self) -> &Arc<AcknowledgeIntegratorAttentionUseCase> {
        &self.acknowledge
    }

    /// The loop's paperwork, as an operator reads it.
    #[must_use]
    pub fn deliveries(&self) -> &Arc<ListAttentionDeliveriesUseCase> {
        &self.deliveries
    }

    /// The walk of the feed the two read paths share with the
    /// subscriber that wakes it.
    #[must_use]
    pub fn recovery(&self) -> &Arc<AttentionRecovery> {
        &self.recovery
    }
}

impl fmt::Debug for IntegratorLoopPorts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("IntegratorLoopPorts").finish()
    }
}

/// The projection one bound integrator's round runs through.
///
/// A free function rather than part of the handle: the append
/// subscriber is installed in the fanout, which is built before the
/// stream the use cases need, so the projection has to exist first and
/// be handed to both.
pub(crate) fn attention_recovery(
    events: Arc<dyn CeremonyEventStorePort>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
    host_delivery: &HostDeliveryPorts,
    systems: &AgenticSystemPorts,
    clock: Arc<dyn ClockPort>,
) -> Arc<AttentionRecovery> {
    Arc::new(AttentionRecovery::new(
        host_delivery.bindings().clone(),
        Arc::new(AttentionAudienceResolver::new(
            systems.executions().clone(),
            systems.repository().clone(),
        )),
        Arc::new(AttentionProjector::new(
            events,
            cursors,
            host_delivery.ledger().clone(),
            host_delivery.activation().clone(),
            clock,
        )),
    ))
}
