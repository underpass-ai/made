//! Resume the protocol outbox before installing its live subscriber.
use crate::ComposeError;
use made_adapters::protocol::{
    ProtocolFanoutMetricsSubscriber, ProtocolMetricsSubscriber, ProtocolStructuredLogSubscriber,
    ProtocolTracingSubscriber,
};
use made_app::services::ProtocolEventPublisherSubscriber;
use made_app::services::{ProtocolEventFanout, SessionStream};
use made_app::usecases::PublishProtocolEventsUseCase;
use made_core::ports::{
    ClockPort, ProtocolEventCursorPort, ProtocolEventStorePort, ProtocolEventSubscriberPort,
    ProtocolEventTransportPort,
};
use std::sync::Arc;

pub(super) async fn wire(
    events: Arc<dyn ProtocolEventStorePort>,
    cursors: Arc<dyn ProtocolEventCursorPort>,
    protocol_transport: Option<Arc<dyn ProtocolEventTransportPort>>,
    clock: Arc<dyn ClockPort>,
) -> Result<Option<Arc<dyn ProtocolEventSubscriberPort>>, ComposeError> {
    let publisher_consumer = made_core::value_objects::ProtocolEventConsumer::new("nats-publisher")
        .expect("the NATS publisher consumer name is valid");
    let publisher_use_case = protocol_transport.map(|transport| {
        Arc::new(PublishProtocolEventsUseCase::new(
            events, cursors, transport, clock,
        ))
    });
    if let Some(publisher) = &publisher_use_case {
        loop {
            let round = publisher
                .execute_automatically(
                    &publisher_consumer,
                    made_core::value_objects::ProtocolEventPageLimit::DEFAULT,
                )
                .await?;
            if round.busy
                || round.confirmed()
                    < made_core::value_objects::ProtocolEventPageLimit::DEFAULT.value()
            {
                break;
            }
        }
    }
    Ok(publisher_use_case.map(|publisher| {
        Arc::new(ProtocolEventPublisherSubscriber::new(
            publisher,
            publisher_consumer,
        )) as Arc<dyn ProtocolEventSubscriberPort>
    }))
}

/// Project sealed events through the same ordered fanout in every service boot.
/// Everything a projection in the engine's own fanout is built from.
///
/// One struct because the list is the composition, and a function that
/// took seven positional ports would be a place to swap two of them by
/// accident and discover it in a projection nobody reads until later.
pub(super) struct EngineProjections {
    pub(super) memory: Arc<dyn ProtocolEventSubscriberPort>,
    pub(super) progress: Arc<dyn ProtocolEventSubscriberPort>,
    pub(super) events: Arc<dyn ProtocolEventStorePort>,
    pub(super) snapshots: Arc<dyn made_core::ports::ProtocolSnapshotStorePort>,
    pub(super) metrics: Arc<dyn made_core::ports::MetricsRecorderPort>,
    pub(super) deliveries: Arc<dyn made_core::ports::HostDeliveryLedgerPort>,
    pub(super) agent_status: Arc<dyn made_core::ports::ProtocolAgentStatusPort>,
    pub(super) attention: Arc<made_app::services::attention::AttentionRecovery>,
}

/// The stream every writer shares, with the engine's own projections
/// hanging off it in one fixed order.
/// What a session leaves behind, projected outside the transaction
/// that sealed it (ADR-012/013).
///
/// Named here rather than spelled into the projection literal because
/// it is the one member of that literal with a decision in it: where
/// the recording happens is the decision, and a literal is not where
/// decisions should be read.
pub(super) fn memory_recorder(
    writer: Arc<dyn made_core::ports::MemoryWriterPort>,
    events: &Arc<dyn made_core::ports::ProtocolEventStorePort>,
) -> Arc<made_app::services::SessionMemoryRecorder> {
    Arc::new(made_app::services::SessionMemoryRecorder::new(
        writer,
        events.clone(),
    ))
}

pub(super) fn stream(
    projections: EngineProjections,
    publisher: Option<Arc<dyn ProtocolEventSubscriberPort>>,
) -> Arc<SessionStream> {
    let events = projections.events.clone();
    let snapshots = projections.snapshots.clone();
    Arc::new(SessionStream::new_authorized(
        events,
        snapshots,
        subscribers(projections, publisher),
    ))
}

fn subscribers(
    projections: EngineProjections,
    publisher: Option<Arc<dyn ProtocolEventSubscriberPort>>,
) -> Arc<dyn ProtocolEventSubscriberPort> {
    let EngineProjections {
        memory,
        progress,
        events,
        snapshots: _,
        metrics,
        deliveries,
        agent_status,
        attention,
    } = projections;
    let mut subscribers: Vec<Arc<dyn ProtocolEventSubscriberPort>> = vec![
        memory,
        progress,
        Arc::new(ProtocolMetricsSubscriber::new(metrics.clone())),
        Arc::new(ProtocolFanoutMetricsSubscriber::new(events, metrics)),
        Arc::new(ProtocolTracingSubscriber::new()),
        Arc::new(ProtocolStructuredLogSubscriber::new()),
        // What is offered to a host is a function of what the stream
        // sealed, in the service exactly as in the embedded engine: a
        // deployment where only one of the two filled the ledger would
        // answer the same question two ways.
        Arc::new(made_app::services::InterventionDeliverySubscriber::new(
            deliveries,
            agent_status,
        )),
        // The integrator's own projection is woken by the same seam.
        // Its cursor is what makes a wake-up nobody received — the
        // process was down — recoverable on the next read.
        Arc::new(made_app::services::attention::AttentionSubscriber::new(
            attention,
        )),
    ];
    subscribers.extend(publisher);
    Arc::new(ProtocolEventFanout::new(subscribers))
}

#[cfg(test)]
mod tests;
