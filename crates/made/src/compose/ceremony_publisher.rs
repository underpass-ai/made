//! Resume the ceremony outbox before installing its live subscriber.
use crate::ComposeError;
use made_app::services::CeremonyEventPublisherSubscriber;
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
pub(super) fn subscribers(
    memory: Arc<dyn CeremonyEventSubscriberPort>,
    progress: Arc<dyn CeremonyEventSubscriberPort>,
    events: Arc<dyn CeremonyEventStorePort>,
    metrics: Arc<dyn made_core::ports::MetricsRecorderPort>,
    publisher: Option<Arc<dyn CeremonyEventSubscriberPort>>,
    deliveries: Arc<dyn made_core::ports::HostDeliveryLedgerPort>,
    agent_status: Arc<dyn made_core::ports::CeremonyAgentStatusPort>,
) -> Arc<dyn CeremonyEventSubscriberPort> {
    use made_adapters::ceremony::{
        CeremonyFanoutMetricsSubscriber, CeremonyMetricsSubscriber,
        CeremonyStructuredLogSubscriber, CeremonyTracingSubscriber,
    };
    use made_app::services::CeremonyEventFanout;
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
    ];
    subscribers.extend(publisher);
    Arc::new(CeremonyEventFanout::new(subscribers))
}
