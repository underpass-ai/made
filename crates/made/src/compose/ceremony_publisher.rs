//! Resume the ceremony outbox before installing its live subscriber.
use crate::ComposeError;
use made_adapters::ceremony::{
    CeremonyFanoutMetricsSubscriber, CeremonyMetricsSubscriber, CeremonyStructuredLogSubscriber,
    CeremonyTracingSubscriber,
};
use made_app::services::CeremonyEventPublisherSubscriber;
use made_app::services::{CeremonyEventFanout, SessionStream};
use made_app::usecases::PublishCeremonyEventsUseCase;
use made_core::ports::{
    CeremonyEventCursorPort, CeremonyEventStorePort, CeremonyEventSubscriberPort,
    CeremonyEventTransportPort, ClockPort,
};
use std::sync::Arc;

pub(super) async fn wire(
    events: Arc<dyn CeremonyEventStorePort>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
    ceremony_transport: Option<Arc<dyn CeremonyEventTransportPort>>,
    clock: Arc<dyn ClockPort>,
) -> Result<Option<Arc<dyn CeremonyEventSubscriberPort>>, ComposeError> {
    let publisher_consumer = made_core::value_objects::CeremonyEventConsumer::new("nats-publisher")
        .expect("the NATS publisher consumer name is valid");
    let publisher_use_case = ceremony_transport.map(|transport| {
        Arc::new(PublishCeremonyEventsUseCase::new(
            events, cursors, transport, clock,
        ))
    });
    if let Some(publisher) = &publisher_use_case {
        loop {
            let round = publisher
                .execute_automatically(
                    &publisher_consumer,
                    made_core::value_objects::CeremonyEventPageLimit::DEFAULT,
                )
                .await?;
            if round.busy
                || round.confirmed()
                    < made_core::value_objects::CeremonyEventPageLimit::DEFAULT.value()
            {
                break;
            }
        }
    }
    Ok(publisher_use_case.map(|publisher| {
        Arc::new(CeremonyEventPublisherSubscriber::new(
            publisher,
            publisher_consumer,
        )) as Arc<dyn CeremonyEventSubscriberPort>
    }))
}

/// Project sealed events through the same ordered fanout in every service boot.
/// Everything a projection in the engine's own fanout is built from.
///
/// One struct because the list is the composition, and a function that
/// took seven positional ports would be a place to swap two of them by
/// accident and discover it in a projection nobody reads until later.
pub(super) struct EngineProjections {
    pub(super) memory: Arc<dyn CeremonyEventSubscriberPort>,
    pub(super) progress: Arc<dyn CeremonyEventSubscriberPort>,
    pub(super) events: Arc<dyn CeremonyEventStorePort>,
    pub(super) snapshots: Arc<dyn made_core::ports::CeremonySnapshotStorePort>,
    pub(super) metrics: Arc<dyn made_core::ports::MetricsRecorderPort>,
    pub(super) deliveries: Arc<dyn made_core::ports::HostDeliveryLedgerPort>,
    pub(super) agent_status: Arc<dyn made_core::ports::CeremonyAgentStatusPort>,
    pub(super) attention: Arc<made_app::services::attention::AttentionRecovery>,
}

/// The stream every writer shares, with the engine's own projections
/// hanging off it in one fixed order.
pub(super) fn stream(
    projections: EngineProjections,
    publisher: Option<Arc<dyn CeremonyEventSubscriberPort>>,
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
    publisher: Option<Arc<dyn CeremonyEventSubscriberPort>>,
) -> Arc<dyn CeremonyEventSubscriberPort> {
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
    let mut subscribers: Vec<Arc<dyn CeremonyEventSubscriberPort>> = vec![
        memory,
        progress,
        Arc::new(CeremonyMetricsSubscriber::new(metrics.clone())),
        Arc::new(CeremonyFanoutMetricsSubscriber::new(events, metrics)),
        Arc::new(CeremonyTracingSubscriber::new()),
        Arc::new(CeremonyStructuredLogSubscriber::new()),
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
    Arc::new(CeremonyEventFanout::new(subscribers))
}
