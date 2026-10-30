use std::sync::Arc;

use async_trait::async_trait;
use made_core::ports::{PositionedRecord, ProtocolEventSubscriberPort};
use made_core::value_objects::{ProtocolEventConsumer, ProtocolEventPageLimit};

use crate::usecases::PublishProtocolEventsUseCase;

/// Wakes a durable publisher after each successful stream append.
pub struct ProtocolEventPublisherSubscriber {
    publisher: Arc<PublishProtocolEventsUseCase>,
    consumer: ProtocolEventConsumer,
}

impl ProtocolEventPublisherSubscriber {
    #[must_use]
    pub fn new(
        publisher: Arc<PublishProtocolEventsUseCase>,
        consumer: ProtocolEventConsumer,
    ) -> Self {
        Self {
            publisher,
            consumer,
        }
    }
}

impl std::fmt::Debug for ProtocolEventPublisherSubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProtocolEventPublisherSubscriber")
            .field("consumer", &self.consumer)
            .finish()
    }
}

#[async_trait]
impl ProtocolEventSubscriberPort for ProtocolEventPublisherSubscriber {
    async fn observe(&self, _records: &[PositionedRecord]) {
        if let Err(error) = self
            .publisher
            .execute_automatically(&self.consumer, ProtocolEventPageLimit::DEFAULT)
            .await
        {
            tracing::warn!(%error, consumer = self.consumer.as_str(), "protocol event publisher drain failed");
        }
    }
}
