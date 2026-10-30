use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_adapters::clock::SystemClock;
use made_adapters::event_sink::JsonLinesProtocolEventSink;
use made_adapters::memory::{InMemoryProtocolEventCursor, InMemoryProtocolEventStore};
use made_app::services::ProtocolEventPublisherSubscriber;
use made_app::usecases::{
    PublishProtocolEventsUseCase, PullProtocolEventsInput, PullProtocolEventsUseCase,
};
use made_core::entities::protocol_events::ProtocolCompleted;
use made_core::entities::{AuditFact, MetricFamily, MetricSample, MetricsSnapshot, ProtocolEvent};
use made_core::error::DomainError;
use made_core::ports::{
    MetricsSnapshotPort, PositionedRecord, ProtocolEventCursorPort, ProtocolEventStorePort,
    ProtocolEventSubscriberPort, ProtocolEventTransportPort,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, EventId, GlobalPosition, MetricHelp, MetricKind, MetricLabelName,
    MetricLabelValue, MetricName, MetricValue, PrometheusText, ProtocolEventConsumer,
    ProtocolEventPageLimit, ProtocolId, ProtocolName, ProtocolVersion, StateId, StreamVersion,
};
use time::OffsetDateTime;

#[derive(Debug, Default)]
struct RecordingTransport {
    failures_remaining: Mutex<usize>,
    attempts: Mutex<Vec<(u64, String)>>,
}

struct SnapshotFake;

impl MetricsSnapshotPort for SnapshotFake {
    fn snapshot(&self) -> Result<MetricsSnapshot, DomainError> {
        let labels = BTreeMap::from([(
            MetricLabelName::new("protocol").unwrap(),
            MetricLabelValue::new("delivery"),
        )]);
        Ok(MetricsSnapshot::new(
            PrometheusText::new(
                "# HELP made_protocol_step_total Completed steps\n# TYPE made_protocol_step_total counter\nmade_protocol_step_total{protocol=\"delivery\"} 1\n",
            ),
            vec![MetricFamily::new(
                MetricName::new("made_protocol_step_total").unwrap(),
                MetricHelp::new("Completed steps"),
                MetricKind::Counter,
                vec![MetricSample::new(
                    MetricName::new("made_protocol_step_total").unwrap(),
                    labels,
                    MetricValue::from_f64(1.0),
                )],
            )],
        ))
    }
}

impl RecordingTransport {
    fn failing(times: usize) -> Self {
        Self {
            failures_remaining: Mutex::new(times),
            attempts: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ProtocolEventTransportPort for RecordingTransport {
    async fn deliver(&self, record: &PositionedRecord) -> Result<(), DomainError> {
        self.attempts.lock().unwrap().push((
            record.position.value(),
            record.record.event_id().as_str().to_owned(),
        ));
        let mut remaining = self.failures_remaining.lock().unwrap();
        if *remaining > 0 {
            *remaining -= 1;
            Err(DomainError::InvariantViolated {
                reason: "deliberate transport failure",
            })
        } else {
            Ok(())
        }
    }
}

fn fact(protocol: &ProtocolId, event: &str) -> AuditFact {
    AuditFact {
        event_id: EventId::new(event).unwrap(),
        event: ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
            final_state: StateId::new("DONE").unwrap(),
            completed_at: OffsetDateTime::UNIX_EPOCH,
        }),
        protocol_id: protocol.clone(),
        definition_name: ProtocolName::new("delivery").unwrap(),
        definition_version: ProtocolVersion::v1(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    }
}

async fn store_with_two_events() -> Arc<InMemoryProtocolEventStore> {
    let store = Arc::new(InMemoryProtocolEventStore::new());
    for (protocol, event) in [("one", "event-1"), ("two", "event-2")] {
        let protocol = ProtocolId::new(protocol).unwrap();
        store
            .append(
                &protocol,
                StreamVersion::EMPTY,
                vec![fact(&protocol, event)],
            )
            .await
            .unwrap();
    }
    store
}

#[tokio::test]
async fn pull_replays_until_ack_and_then_resumes_after_it() {
    let store = store_with_two_events().await;
    let cursors = Arc::new(InMemoryProtocolEventCursor::new());
    let pull = PullProtocolEventsUseCase::new(store, cursors.clone());
    let consumer = ProtocolEventConsumer::new("dashboard").unwrap();
    let limit = ProtocolEventPageLimit::new(1).unwrap();

    let first = pull
        .execute(PullProtocolEventsInput::new(consumer.clone(), limit, None))
        .await
        .unwrap();
    let replay = pull
        .execute(PullProtocolEventsInput::new(consumer.clone(), limit, None))
        .await
        .unwrap();
    assert_eq!(first.records(), replay.records());
    assert_eq!(cursors.position(&consumer).await.unwrap(), None);

    let second = pull
        .execute(PullProtocolEventsInput::new(
            consumer.clone(),
            limit,
            Some(GlobalPosition::FIRST),
        ))
        .await
        .unwrap();
    assert_eq!(
        second.records()[0].position,
        GlobalPosition::new(2).unwrap()
    );
    assert_eq!(second.acknowledged_through(), Some(GlobalPosition::FIRST));
}

#[tokio::test]
async fn publisher_retries_the_same_event_id_before_advancing() {
    let store = store_with_two_events().await;
    let cursors = Arc::new(InMemoryProtocolEventCursor::new());
    let transport = Arc::new(RecordingTransport::failing(1));
    let publisher = PublishProtocolEventsUseCase::new(
        store,
        cursors.clone(),
        transport.clone(),
        Arc::new(SystemClock::new()),
    );
    let consumer = ProtocolEventConsumer::new("publisher").unwrap();

    let failed = publisher
        .execute(&consumer, ProtocolEventPageLimit::DEFAULT)
        .await
        .unwrap();
    assert_eq!(failed.failed, 1);
    assert_eq!(cursors.position(&consumer).await.unwrap(), None);
    let delivered = publisher
        .execute(&consumer, ProtocolEventPageLimit::DEFAULT)
        .await
        .unwrap();
    assert_eq!(delivered.delivered, 2);
    let attempts = transport.attempts.lock().unwrap();
    assert_eq!(attempts[0], attempts[1]);
    assert_eq!(attempts[2].0, 2);
}

#[tokio::test(start_paused = true)]
async fn automatic_subscriber_retries_without_another_append() {
    let store = store_with_two_events().await;
    let cursors = Arc::new(InMemoryProtocolEventCursor::new());
    let transport = Arc::new(RecordingTransport::failing(1));
    let consumer = ProtocolEventConsumer::new("automatic-publisher").unwrap();
    let subscriber = ProtocolEventPublisherSubscriber::new(
        Arc::new(PublishProtocolEventsUseCase::new(
            store,
            cursors.clone(),
            transport.clone(),
            Arc::new(SystemClock::new()),
        )),
        consumer.clone(),
    );

    // One notification only. The transport recovers after its first failure;
    // no later protocol append is available to wake publication again.
    subscriber.observe(&[]).await;

    assert_eq!(
        cursors.position(&consumer).await.unwrap(),
        Some(GlobalPosition::new(2).unwrap())
    );
    let attempts = transport.attempts.lock().unwrap();
    assert_eq!(attempts.len(), 3);
    assert_eq!(attempts[0], attempts[1]);
    assert_eq!(attempts[2].0, 2);
}

#[tokio::test]
async fn a_three_time_failure_is_quarantined_and_made_visible() {
    let store = store_with_two_events().await;
    let cursors = Arc::new(InMemoryProtocolEventCursor::new());
    let publisher = PublishProtocolEventsUseCase::new(
        store,
        cursors.clone(),
        Arc::new(RecordingTransport::failing(usize::MAX)),
        Arc::new(SystemClock::new()),
    );
    let consumer = ProtocolEventConsumer::new("quarantine-test").unwrap();

    for _ in 0..3 {
        assert_eq!(
            publisher
                .execute(&consumer, ProtocolEventPageLimit::DEFAULT)
                .await
                .unwrap()
                .failed,
            1
        );
    }
    let fourth = publisher
        .execute(&consumer, ProtocolEventPageLimit::new(1).unwrap())
        .await
        .unwrap();
    assert_eq!(fourth.quarantined, 1);
    let quarantine = cursors.quarantined(&consumer).await.unwrap();
    assert_eq!(quarantine.len(), 1);
    assert_eq!(quarantine[0].position(), GlobalPosition::FIRST);
}

#[tokio::test(start_paused = true)]
async fn automatic_publisher_reports_retries_separately_from_quarantine() {
    let store = store_with_two_events().await;
    let cursors = Arc::new(InMemoryProtocolEventCursor::new());
    let publisher = PublishProtocolEventsUseCase::new(
        store,
        cursors.clone(),
        Arc::new(RecordingTransport::failing(usize::MAX)),
        Arc::new(SystemClock::new()),
    );
    let consumer = ProtocolEventConsumer::new("automatic-quarantine").unwrap();

    let summary = publisher
        .execute_automatically(&consumer, ProtocolEventPageLimit::new(1).unwrap())
        .await
        .unwrap();

    assert_eq!(summary.retried, 3);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.delivered, 0);
    assert_eq!(summary.quarantined, 1);
    assert_eq!(summary.confirmed(), 1);
    assert_eq!(
        cursors.position(&consumer).await.unwrap(),
        Some(GlobalPosition::FIRST)
    );
}

#[tokio::test(start_paused = true)]
async fn automatic_page_limit_counts_confirmed_positions_not_failed_attempts() {
    let store = store_with_two_events().await;
    let cursors = Arc::new(InMemoryProtocolEventCursor::new());
    let transport = Arc::new(RecordingTransport::failing(1));
    let publisher = PublishProtocolEventsUseCase::new(
        store,
        cursors.clone(),
        transport.clone(),
        Arc::new(SystemClock::new()),
    );
    let consumer = ProtocolEventConsumer::new("bounded-automatic-publisher").unwrap();

    let summary = publisher
        .execute_automatically(&consumer, ProtocolEventPageLimit::new(1).unwrap())
        .await
        .unwrap();

    assert_eq!(summary.delivered, 1);
    assert_eq!(summary.retried, 1);
    assert_eq!(summary.confirmed(), 1);
    assert_eq!(
        cursors.position(&consumer).await.unwrap(),
        Some(GlobalPosition::FIRST)
    );
    let attempts = transport.attempts.lock().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0], attempts[1]);
    assert!(attempts.iter().all(|attempt| attempt.0 == 1));
}

#[tokio::test]
async fn json_lines_sink_appends_complete_positioned_records() {
    let store = store_with_two_events().await;
    let record = store
        .read_all(
            GlobalPosition::FIRST,
            ProtocolEventPageLimit::new(1).unwrap(),
        )
        .await
        .unwrap()
        .remove(0);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("protocol-events.jsonl");
    let sink = JsonLinesProtocolEventSink::open(&path).unwrap();

    sink.deliver(&record).await.unwrap();

    let line = std::fs::read_to_string(path).unwrap();
    assert_eq!(line.lines().count(), 1);
    let json: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(json["global_position"], 1);
    assert_eq!(json["event_id"], "event-1");
    assert_eq!(json["event_type"], "protocol_completed");
    assert!(json["record_hash"].is_array());
}

#[tokio::test]
async fn observable_json_lines_sink_appends_the_event_and_registry_snapshot_together() {
    let store = store_with_two_events().await;
    let record = store
        .read_all(
            GlobalPosition::FIRST,
            ProtocolEventPageLimit::new(1).unwrap(),
        )
        .await
        .unwrap()
        .remove(0);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("protocol-events.jsonl");
    let sink =
        JsonLinesProtocolEventSink::open_with_metrics(&path, Arc::new(SnapshotFake)).unwrap();

    sink.deliver(&record).await.unwrap();

    let contents = std::fs::read_to_string(path).unwrap();
    let lines = contents.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    let event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(event["global_position"], 1);
    assert_eq!(event["event_id"], "event-1");
    let snapshot: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(snapshot["record_type"], "metrics_snapshot");
    assert!(snapshot["registry_text"]
        .as_str()
        .unwrap()
        .contains("made_protocol_step_total"));
    assert_eq!(snapshot["registry"][0]["name"], "made_protocol_step_total");
    assert_eq!(snapshot["registry"][0]["samples"][0]["value"], 1.0);
}
