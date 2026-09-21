//! Everything the embedded engine projects off its own stream, in one
//! fixed order.
//!
//! One struct because the list is the composition, and a function that
//! took seven positional ports would be a place to swap two of them by
//! accident and discover it in a projection nobody reads until later.
//! The same list the deployable service installs: a deployment where
//! only one of the two filled the delivery ledger would answer the
//! same question two ways.

use std::sync::Arc;

use made_adapters::ceremony::{
    CeremonyFanoutMetricsSubscriber, CeremonyMetricsSubscriber, CeremonyStructuredLogSubscriber,
    CeremonyTracingSubscriber,
};
use made_app::services::attention::{AttentionRecovery, AttentionSubscriber};
use made_app::services::{
    CeremonyEventFanout, InterventionDeliverySubscriber, SessionMemoryRecorder,
};
use made_core::ports::{
    CeremonyAgentStatusPort, CeremonyEventStorePort, CeremonyEventSubscriberPort,
    HostDeliveryLedgerPort, MemoryWriterPort, MetricsRecorderPort,
};

/// The ports the engine's own projections are built from.
pub(crate) struct EngineProjections {
    pub(crate) memory: Arc<dyn MemoryWriterPort>,
    pub(crate) events: Arc<dyn CeremonyEventStorePort>,
    pub(crate) progress: Arc<dyn CeremonyEventSubscriberPort>,
    pub(crate) metrics: Arc<dyn MetricsRecorderPort>,
    pub(crate) deliveries: Arc<dyn HostDeliveryLedgerPort>,
    pub(crate) agent_status: Arc<dyn CeremonyAgentStatusPort>,
    pub(crate) attention: Arc<AttentionRecovery>,
}

/// The engine's projections, then anything the host asked to add.
///
/// A host's own subscriber comes last, after everything the engine
/// needs for its own answers to be right.
pub(crate) fn fanout(
    projections: EngineProjections,
    host: Vec<Arc<dyn CeremonyEventSubscriberPort>>,
) -> Arc<dyn CeremonyEventSubscriberPort> {
    let EngineProjections {
        memory,
        events,
        progress,
        metrics,
        deliveries,
        agent_status,
        attention,
    } = projections;
    let mut subscribers: Vec<Arc<dyn CeremonyEventSubscriberPort>> = vec![
        // What a session leaves behind is a projection of its stream,
        // so it is a subscriber rather than something a use case
        // holds. A host that configures no memory gets one that
        // forgets and says so; handing in a durable writer is the
        // whole of turning it on.
        Arc::new(SessionMemoryRecorder::new(memory, events.clone())),
        progress,
        Arc::new(CeremonyMetricsSubscriber::new(metrics.clone())),
        Arc::new(CeremonyFanoutMetricsSubscriber::new(events, metrics)),
        Arc::new(CeremonyTracingSubscriber::new()),
        Arc::new(CeremonyStructuredLogSubscriber::new()),
        // What is offered to a host is a function of what the stream
        // sealed, so the ledger is filled by being told rather than by
        // each writer remembering to.
        Arc::new(InterventionDeliverySubscriber::new(
            deliveries,
            agent_status,
        )),
        // The integrator's own projection is woken by the same seam.
        // Its cursor is what makes a wake-up nobody received — the
        // process was down — recoverable on the next read.
        Arc::new(AttentionSubscriber::new(attention)),
    ];
    subscribers.extend(host);
    Arc::new(CeremonyEventFanout::new(subscribers))
}
