use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_adapters::memory::{InMemoryProtocolEventCursor, InMemoryProtocolEventStore};
use made_core::entities::protocol_events::ProtocolCompleted;
use made_core::entities::{AuditFact, ProtocolEvent};
use made_core::error::DomainError;
use made_core::ports::{
    PositionedRecord, ProtocolEventCursorPort, ProtocolEventStorePort, ProtocolEventTransportPort,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, EventId, GlobalPosition, ProtocolEventConsumer, ProtocolId,
    ProtocolName, ProtocolVersion, StateId, StreamVersion,
};
use made_embedded::EmbeddedMade;
use time::OffsetDateTime;

#[derive(Debug)]
struct RecoveringTransport {
    failures_remaining: Mutex<usize>,
    attempts: Mutex<Vec<String>>,
}

impl RecoveringTransport {
    fn fails_once() -> Self {
        Self {
            failures_remaining: Mutex::new(1),
            attempts: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ProtocolEventTransportPort for RecoveringTransport {
    async fn deliver(&self, record: &PositionedRecord) -> Result<(), DomainError> {
        self.attempts
            .lock()
            .unwrap()
            .push(record.record.event_id().as_str().to_owned());
        let mut remaining = self.failures_remaining.lock().unwrap();
        if *remaining == 0 {
            Ok(())
        } else {
            *remaining -= 1;
            Err(DomainError::InvariantViolated {
                reason: "deliberate startup delivery failure",
            })
        }
    }
}

fn fact(protocol_id: &ProtocolId) -> AuditFact {
    AuditFact {
        event_id: EventId::new("pending-startup-publication").unwrap(),
        event: ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
            final_state: StateId::new("DONE").unwrap(),
            completed_at: OffsetDateTime::UNIX_EPOCH,
        }),
        protocol_id: protocol_id.clone(),
        definition_name: ProtocolName::new("startup_publication").unwrap(),
        definition_version: ProtocolVersion::v1(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    }
}

#[tokio::test]
async fn startup_recovery_retries_without_a_new_append() {
    let protocol_id = ProtocolId::new("pending-startup-publication").unwrap();
    let store = Arc::new(InMemoryProtocolEventStore::new());
    store
        .append(&protocol_id, StreamVersion::EMPTY, vec![fact(&protocol_id)])
        .await
        .unwrap();
    let cursor = Arc::new(InMemoryProtocolEventCursor::new());
    let transport = Arc::new(RecoveringTransport::fails_once());
    let made = EmbeddedMade::builder()
        .with_protocol_store(store)
        .with_event_cursor(cursor.clone())
        .with_event_transport(transport.clone())
        .build();

    made.recover_event_publication().await.unwrap();

    let consumer = ProtocolEventConsumer::new("embedded-file-sink").unwrap();
    assert_eq!(
        cursor.position(&consumer).await.unwrap(),
        Some(GlobalPosition::FIRST)
    );
    let attempts = transport.attempts.lock().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0], "pending-startup-publication");
    assert_eq!(attempts[0], attempts[1]);
}
