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
