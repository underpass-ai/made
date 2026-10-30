use std::sync::Arc;

use async_nats::Client;
use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{PositionedRecord, ProtocolEventTransportPort};

use crate::protocol_event_wire::ProtocolEventWire;

use super::nats_publish_client::NatsPublishClient;
use super::NatsSubjects;

const DELIVERY_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// Publishes complete protocol records on subjects grouped by event type.
#[derive(Clone)]
pub struct NatsProtocolEventTransport {
    client: Arc<dyn NatsPublishClient>,
    subjects: NatsSubjects,
}

impl NatsProtocolEventTransport {
    #[must_use]
    pub fn new(client: Client, subjects: NatsSubjects) -> Self {
        Self::from_client(Arc::new(client), subjects)
    }

    fn from_client(client: Arc<dyn NatsPublishClient>, subjects: NatsSubjects) -> Self {
        Self { client, subjects }
    }
}

impl std::fmt::Debug for NatsProtocolEventTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NatsProtocolEventTransport")
            .field("subjects", &self.subjects)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ProtocolEventTransportPort for NatsProtocolEventTransport {
    async fn deliver(&self, record: &PositionedRecord) -> Result<(), DomainError> {
        let payload = serde_json::to_vec(&ProtocolEventWire::from(record)).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "nats: failed to serialize protocol event",
            }
        })?;
        let subject = format!(
            "{}.{}",
            self.subjects.protocol_prefix,
            record.record.event_type().as_str()
        );
        tokio::time::timeout(DELIVERY_DEADLINE, async {
            self.client.publish(subject, payload).await.map_err(|()| {
                DomainError::InvariantViolated {
                    reason: "nats: protocol event publish failed",
                }
            })?;
            // `publish` only queues a command in async-nats. `flush` waits
            // until the connection task has drained that client buffer to
            // its transport. In async-nats 0.50 it does not wait for a PONG,
            // a subscriber acknowledgement or JetStream persistence.
            self.client
                .flush()
                .await
                .map_err(|()| DomainError::InvariantViolated {
                    reason: "nats: protocol event client flush failed",
                })
        })
        .await
        .map_err(|_| DomainError::InvariantViolated {
            reason: "nats: protocol event delivery timed out",
        })?
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use made_app::usecases::PublishProtocolEventsUseCase;
    use made_core::entities::protocol_events::ProtocolCompleted;
    use made_core::entities::{AuditFact, ProtocolEvent};
    use made_core::ports::{ProtocolEventCursorPort, ProtocolEventStorePort};
    use made_core::value_objects::{
        AuditActor, AuditActorKind, EventId, GlobalPosition, ProtocolEventConsumer,
        ProtocolEventPageLimit, ProtocolId, ProtocolName, ProtocolVersion, StateId, StreamVersion,
    };
    use time::OffsetDateTime;

    use crate::clock::SystemClock;
    use crate::memory::{InMemoryProtocolEventCursor, InMemoryProtocolEventStore};

    use super::*;

    #[derive(Debug)]
    struct ConfirmationClient {
        flush_failures: AtomicUsize,
        flush_hangs: bool,
        publishes: AtomicUsize,
        flushes: AtomicUsize,
    }

    impl ConfirmationClient {
        fn failing_flushes(count: usize) -> Self {
            Self {
                flush_failures: AtomicUsize::new(count),
                flush_hangs: false,
                publishes: AtomicUsize::new(0),
                flushes: AtomicUsize::new(0),
            }
        }

        fn hanging_flush() -> Self {
            Self {
                flush_failures: AtomicUsize::new(0),
                flush_hangs: true,
                publishes: AtomicUsize::new(0),
                flushes: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl NatsPublishClient for ConfirmationClient {
        async fn publish(&self, _subject: String, _payload: Vec<u8>) -> Result<(), ()> {
            self.publishes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn flush(&self) -> Result<(), ()> {
            self.flushes.fetch_add(1, Ordering::SeqCst);
            if self.flush_hangs {
                std::future::pending::<()>().await;
            }
            self.flush_failures
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .map_or(Ok(()), |_| Err(()))
        }
    }

    fn fact(protocol_id: &ProtocolId) -> AuditFact {
        AuditFact {
            event_id: EventId::new("flush-confirmation-event").unwrap(),
            event: ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
                final_state: StateId::new("DONE").unwrap(),
                completed_at: OffsetDateTime::UNIX_EPOCH,
            }),
            protocol_id: protocol_id.clone(),
            definition_name: ProtocolName::new("flush_confirmation").unwrap(),
            definition_version: ProtocolVersion::v1(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        }
    }

    #[tokio::test]
    async fn failed_client_flush_keeps_the_cursor_pending() {
        let protocol_id = ProtocolId::new("flush-confirmation").unwrap();
        let events = Arc::new(InMemoryProtocolEventStore::new());
        events
            .append(&protocol_id, StreamVersion::EMPTY, vec![fact(&protocol_id)])
            .await
            .unwrap();
        let cursors = Arc::new(InMemoryProtocolEventCursor::new());
        let client = Arc::new(ConfirmationClient::failing_flushes(1));
        let transport = Arc::new(NatsProtocolEventTransport::from_client(
            client.clone(),
            NatsSubjects::new("made", "made.trigger.>").unwrap(),
        ));
        let publisher = PublishProtocolEventsUseCase::new(
            events,
            cursors.clone(),
            transport,
            Arc::new(SystemClock::new()),
        );
        let consumer = ProtocolEventConsumer::new("flush-confirmation").unwrap();

        let failed = publisher
            .execute(&consumer, ProtocolEventPageLimit::new(1).unwrap())
            .await
            .unwrap();

        assert_eq!(failed.failed, 1);
        assert_eq!(cursors.position(&consumer).await.unwrap(), None);
        assert_eq!(client.publishes.load(Ordering::SeqCst), 1);
        assert_eq!(client.flushes.load(Ordering::SeqCst), 1);

        let delivered = publisher
            .execute(&consumer, ProtocolEventPageLimit::new(1).unwrap())
            .await
            .unwrap();
        assert_eq!(delivered.delivered, 1);
        assert_eq!(
            cursors.position(&consumer).await.unwrap(),
            Some(GlobalPosition::FIRST)
        );
        assert_eq!(client.publishes.load(Ordering::SeqCst), 2);
        assert_eq!(client.flushes.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_hung_client_flush_times_out_and_keeps_the_cursor_pending() {
        let protocol_id = ProtocolId::new("hung-flush").unwrap();
        let events = Arc::new(InMemoryProtocolEventStore::new());
        events
            .append(&protocol_id, StreamVersion::EMPTY, vec![fact(&protocol_id)])
            .await
            .unwrap();
        let cursors = Arc::new(InMemoryProtocolEventCursor::new());
        let client = Arc::new(ConfirmationClient::hanging_flush());
        let transport = Arc::new(NatsProtocolEventTransport::from_client(
            client.clone(),
            NatsSubjects::new("made", "made.trigger.>").unwrap(),
        ));
        let publisher = PublishProtocolEventsUseCase::new(
            events,
            cursors.clone(),
            transport,
            Arc::new(SystemClock::new()),
        );
        let consumer = ProtocolEventConsumer::new("hung-flush").unwrap();

        let failed = publisher
            .execute(&consumer, ProtocolEventPageLimit::new(1).unwrap())
            .await
            .unwrap();

        assert_eq!(failed.failed, 1);
        assert_eq!(cursors.position(&consumer).await.unwrap(), None);
        assert_eq!(client.publishes.load(Ordering::SeqCst), 1);
        assert_eq!(client.flushes.load(Ordering::SeqCst), 1);
    }
}
