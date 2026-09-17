use std::sync::Arc;

use async_trait::async_trait;
use made_core::ports::{CeremonyEventSubscriberPort, PositionedRecord};
use made_core::value_objects::{CeremonyEventConsumer, CeremonyEventPageLimit};

use crate::usecases::PublishCeremonyEventsUseCase;

/// Wakes a durable publisher after each successful stream append.
pub struct CeremonyEventPublisherSubscriber {
    publisher: Arc<PublishCeremonyEventsUseCase>,
    consumer: CeremonyEventConsumer,
}

impl CeremonyEventPublisherSubscriber {
    #[must_use]
    pub fn new(
        publisher: Arc<PublishCeremonyEventsUseCase>,
        consumer: CeremonyEventConsumer,
    ) -> Self {
        Self {
            publisher,
            consumer,
        }
    }
}

impl std::fmt::Debug for CeremonyEventPublisherSubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyEventPublisherSubscriber")
            .field("consumer", &self.consumer)
            .finish()
    }
}

#[async_trait]
impl CeremonyEventSubscriberPort for CeremonyEventPublisherSubscriber {
    async fn observe(&self, _records: &[PositionedRecord]) {
        if let Err(error) = self
            .publisher
            .execute(&self.consumer, CeremonyEventPageLimit::DEFAULT)
            .await
        {
            tracing::warn!(%error, consumer = self.consumer.as_str(), "ceremony event publisher drain failed");
        }
    }
}
