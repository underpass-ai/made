use std::sync::Arc;

use async_nats::Client;
use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{CeremonyEventTransportPort, PositionedRecord};

use crate::ceremony_event_wire::CeremonyEventWire;

use super::nats_publish_client::NatsPublishClient;
use super::NatsSubjects;

const DELIVERY_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// Publishes complete ceremony records on subjects grouped by event type.
#[derive(Clone)]
pub struct NatsCeremonyEventTransport {
    client: Arc<dyn NatsPublishClient>,
    subjects: NatsSubjects,
}

impl NatsCeremonyEventTransport {
    #[must_use]
    pub fn new(client: Client, subjects: NatsSubjects) -> Self {
        Self::from_client(Arc::new(client), subjects)
    }

    fn from_client(client: Arc<dyn NatsPublishClient>, subjects: NatsSubjects) -> Self {
        Self { client, subjects }
    }
}

impl std::fmt::Debug for NatsCeremonyEventTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NatsCeremonyEventTransport")
            .field("subjects", &self.subjects)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl CeremonyEventTransportPort for NatsCeremonyEventTransport {
    async fn deliver(&self, record: &PositionedRecord) -> Result<(), DomainError> {
        let payload = serde_json::to_vec(&CeremonyEventWire::from(record)).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "nats: failed to serialize ceremony event",
            }
        })?;
        let subject = format!(
            "{}.{}",
            self.subjects.ceremony_prefix,
            record.record.event_type().as_str()
        );
        tokio::time::timeout(DELIVERY_DEADLINE, async {
            self.client.publish(subject, payload).await.map_err(|()| {
                DomainError::InvariantViolated {
                    reason: "nats: ceremony event publish failed",
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
                    reason: "nats: ceremony event client flush failed",
                })
        })
        .await
        .map_err(|_| DomainError::InvariantViolated {
            reason: "nats: ceremony event delivery timed out",
        })?
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use made_app::usecases::PublishCeremonyEventsUseCase;
    use made_core::entities::ceremony_events::CeremonyCompleted;
    use made_core::entities::{AuditFact, CeremonyEvent};
    use made_core::ports::{CeremonyEventCursorPort, CeremonyEventStorePort};
    use made_core::value_objects::{
        AuditActor, AuditActorKind, CeremonyEventConsumer, CeremonyEventPageLimit, CeremonyId,
        CeremonyName, CeremonyVersion, EventId, GlobalPosition, StateId, StreamVersion,
    };
    use time::OffsetDateTime;

    use crate::clock::SystemClock;
    use crate::memory::{InMemoryCeremonyEventCursor, InMemoryCeremonyEventStore};

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

    fn fact(ceremony_id: &CeremonyId) -> AuditFact {
        AuditFact {
            event_id: EventId::new("flush-confirmation-event").unwrap(),
            event: CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
                final_state: StateId::new("DONE").unwrap(),
                completed_at: OffsetDateTime::UNIX_EPOCH,
            }),
            ceremony_id: ceremony_id.clone(),
            definition_name: CeremonyName::new("flush_confirmation").unwrap(),
            definition_version: CeremonyVersion::v1(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        }
    }

    #[tokio::test]
    async fn failed_client_flush_keeps_the_cursor_pending() {
        let ceremony_id = CeremonyId::new("flush-confirmation").unwrap();
        let events = Arc::new(InMemoryCeremonyEventStore::new());
        events
            .append(&ceremony_id, StreamVersion::EMPTY, vec![fact(&ceremony_id)])
            .await
            .unwrap();
        let cursors = Arc::new(InMemoryCeremonyEventCursor::new());
        let client = Arc::new(ConfirmationClient::failing_flushes(1));
        let transport = Arc::new(NatsCeremonyEventTransport::from_client(
            client.clone(),
            NatsSubjects::new("made", "made.trigger.>").unwrap(),
        ));
        let publisher = PublishCeremonyEventsUseCase::new(
            events,
            cursors.clone(),
            transport,
            Arc::new(SystemClock::new()),
        );
        let consumer = CeremonyEventConsumer::new("flush-confirmation").unwrap();

        let failed = publisher
            .execute(&consumer, CeremonyEventPageLimit::new(1).unwrap())
            .await
            .unwrap();

        assert_eq!(failed.failed, 1);
        assert_eq!(cursors.position(&consumer).await.unwrap(), None);
        assert_eq!(client.publishes.load(Ordering::SeqCst), 1);
        assert_eq!(client.flushes.load(Ordering::SeqCst), 1);

        let delivered = publisher
            .execute(&consumer, CeremonyEventPageLimit::new(1).unwrap())
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
        let ceremony_id = CeremonyId::new("hung-flush").unwrap();
        let events = Arc::new(InMemoryCeremonyEventStore::new());
        events
            .append(&ceremony_id, StreamVersion::EMPTY, vec![fact(&ceremony_id)])
            .await
            .unwrap();
        let cursors = Arc::new(InMemoryCeremonyEventCursor::new());
        let client = Arc::new(ConfirmationClient::hanging_flush());
        let transport = Arc::new(NatsCeremonyEventTransport::from_client(
            client.clone(),
            NatsSubjects::new("made", "made.trigger.>").unwrap(),
        ));
        let publisher = PublishCeremonyEventsUseCase::new(
            events,
            cursors.clone(),
            transport,
            Arc::new(SystemClock::new()),
        );
        let consumer = CeremonyEventConsumer::new("hung-flush").unwrap();

        let failed = publisher
            .execute(&consumer, CeremonyEventPageLimit::new(1).unwrap())
            .await
            .unwrap();

        assert_eq!(failed.failed, 1);
        assert_eq!(cursors.position(&consumer).await.unwrap(), None);
        assert_eq!(client.publishes.load(Ordering::SeqCst), 1);
        assert_eq!(client.flushes.load(Ordering::SeqCst), 1);
    }
}
