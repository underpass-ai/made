use std::collections::BTreeSet;
use std::sync::Arc;

use made_core::entities::protocol_events::{
    ProtocolCompleted, ProtocolInstanceStarted, StateIterationStarted, StepCompleted, StepFailed,
    StepStarted,
};
use made_core::entities::{AuditFact, ProtocolEvent};
use made_core::error::DomainError;
use made_core::ports::{ProtocolEventStorePort, ProtocolEventSubscriberPort};
use made_core::value_objects::*;
use time::{Duration, OffsetDateTime};

use super::ProtocolFanoutMetricsSubscriber;
use crate::memory::InMemoryProtocolEventStore;
use crate::metrics::PrometheusMetricsRecorder;

fn start() -> ProtocolEvent {
    ProtocolEvent::ProtocolInstanceStarted(ProtocolInstanceStarted {
        protocol_id: ProtocolId::new("fanout").unwrap(),
        definition_name: ProtocolName::new("review").unwrap(),
        definition_version: ProtocolVersion::v1(),
        initial_state: StateId::new("REVIEW").unwrap(),
        step_ids: BTreeSet::from([StepId::new("a").unwrap(), StepId::new("b").unwrap()]),
        context: ProtocolContext::empty(),
        bound_definition: None,
        lineage: None,
        succession: None,
        budget_account_id: None,
        protocol_deadline: None,
        state_deadline: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    })
}

fn claim(step: &str, at: i64, iteration: StateIteration) -> ProtocolEvent {
    let now = OffsetDateTime::UNIX_EPOCH + Duration::seconds(at);
    ProtocolEvent::StepStarted(StepStarted {
        step_id: StepId::new(step).unwrap(),
        state_visit: Some(StateVisit::FIRST),
        state_iteration: Some(iteration),
        iteration: StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        lease: StepLease::acquire(
            LeaseOwnerId::new(step).unwrap(),
            IdempotencyKey::new(format!("{step}-{at}")).unwrap(),
            now,
            DurationMs::from_millis(10_000),
        )
        .unwrap(),
        started_by: RoleId::new(step).unwrap(),
        role_from: None,
        sealed_role: None,
        deadline: None,
        budget_reservation_id: None,
        started_at: now,
    })
}

fn finish(step: &str, at: i64, iteration: StateIteration) -> ProtocolEvent {
    ProtocolEvent::StepCompleted(StepCompleted {
        step_id: StepId::new(step).unwrap(),
        state_visit: Some(StateVisit::FIRST),
        state_iteration: Some(iteration),
        iteration: StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        next_iteration: None,
        finished_by: RoleId::new(step).unwrap(),
        finished_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(at),
    })
}

fn done(at: i64) -> ProtocolEvent {
    ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
        final_state: StateId::new("DONE").unwrap(),
        completed_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(at),
    })
}

async fn store(events: Vec<ProtocolEvent>) -> Arc<InMemoryProtocolEventStore> {
    let store = Arc::new(InMemoryProtocolEventStore::new());
    let facts = events
        .into_iter()
        .enumerate()
        .map(|(index, event)| AuditFact {
            event_id: EventId::new(format!("event-{index}")).unwrap(),
            event,
            protocol_id: ProtocolId::new("fanout").unwrap(),
            definition_name: ProtocolName::new("review").unwrap(),
            definition_version: ProtocolVersion::v1(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        })
        .collect();
    store
        .append(
            &ProtocolId::new("fanout").unwrap(),
            StreamVersion::EMPTY,
            facts,
        )
        .await
        .unwrap();
    store
}

#[tokio::test]
async fn out_of_order_and_duplicate_notifications_project_one_peak_and_typed_failure() {
    let failed = ProtocolEvent::StepFailed(StepFailed {
        step_id: StepId::new("a").unwrap(),
        state_visit: Some(StateVisit::FIRST),
        state_iteration: Some(StateIteration::FIRST),
        iteration: StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        result: StepResult::from_handler_error(&DomainError::NoValidProposal {
            contract_id: "review".to_owned(),
        })
        .unwrap(),
        finished_by: RoleId::new("a").unwrap(),
        finished_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(3),
    });
    let store = store(vec![
        start(),
        claim("a", 1, StateIteration::FIRST),
        claim("b", 2, StateIteration::FIRST),
        finish("b", 3, StateIteration::FIRST),
        failed,
        done(4),
    ])
    .await;
    let records = store
        .read_all(GlobalPosition::FIRST, ProtocolEventPageLimit::DEFAULT)
        .await
        .unwrap();
    let metrics = Arc::new(PrometheusMetricsRecorder::new().unwrap());
    let subscriber = ProtocolFanoutMetricsSubscriber::new(store, metrics.clone());
    subscriber.observe(&records[records.len() - 1..]).await;
    subscriber.observe(&records[..2]).await;
    subscriber.observe(&records).await;
    let text = metrics.render().unwrap();
    assert!(
        text.contains("made_protocol_claim_peak_width_sum{protocol=\"review\",state=\"REVIEW\"} 2"),
        "{text}"
    );
    assert!(
        text.contains(
            "made_protocol_claim_peak_width_count{protocol=\"review\",state=\"REVIEW\"} 1"
        ),
        "{text}"
    );
    assert!(text.contains("made_protocol_step_failure_total{protocol=\"review\",failure_kind=\"no_valid_proposal\",step=\"a\"} 1"), "{text}");
}

#[tokio::test]
async fn expiry_and_state_repeat_do_not_inflate_peak() {
    let second = StateIteration::new(2).unwrap();
    let repeat = ProtocolEvent::StateIterationStarted(StateIterationStarted {
        state_id: StateId::new("REVIEW").unwrap(),
        state_visit: Some(StateVisit::FIRST),
        state_iteration: second,
        step_ids: vec![StepId::new("a").unwrap(), StepId::new("b").unwrap()],
        started_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(13),
    });
    let store = store(vec![
        start(),
        claim("a", 1, StateIteration::FIRST),
        claim("b", 12, StateIteration::FIRST),
        finish("b", 13, StateIteration::FIRST),
        repeat,
        claim("a", 14, second),
        claim("b", 15, second),
        finish("a", 16, second),
        finish("b", 17, second),
        done(18),
    ])
    .await;
    let records = store
        .read_all(GlobalPosition::FIRST, ProtocolEventPageLimit::DEFAULT)
        .await
        .unwrap();
    let metrics = Arc::new(PrometheusMetricsRecorder::new().unwrap());
    let subscriber = ProtocolFanoutMetricsSubscriber::new(store, metrics.clone());
    subscriber.observe(&records).await;
    let text = metrics.render().unwrap();
    assert!(
        text.contains("made_protocol_claim_peak_width_sum{protocol=\"review\",state=\"REVIEW\"} 3"),
        "{text}"
    );
    assert!(
        text.contains(
            "made_protocol_claim_peak_width_count{protocol=\"review\",state=\"REVIEW\"} 2"
        ),
        "{text}"
    );
    assert!(!text.contains("made_protocol_step_failure_total{"));
}

#[tokio::test]
async fn classified_step_failure_also_counts_single_claim_work() {
    let failed = ProtocolEvent::StepFailed(StepFailed {
        step_id: StepId::new("a").unwrap(),
        state_visit: Some(StateVisit::FIRST),
        state_iteration: Some(StateIteration::FIRST),
        iteration: StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        result: StepResult::from_handler_error(&DomainError::NoValidProposal {
            contract_id: "review".to_owned(),
        })
        .unwrap(),
        finished_by: RoleId::new("a").unwrap(),
        finished_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2),
    });
    let store = store(vec![
        start(),
        claim("a", 1, StateIteration::FIRST),
        failed,
        done(3),
    ])
    .await;
    let records = store
        .read_all(GlobalPosition::FIRST, ProtocolEventPageLimit::DEFAULT)
        .await
        .unwrap();
    let metrics = Arc::new(PrometheusMetricsRecorder::new().unwrap());
    let subscriber = ProtocolFanoutMetricsSubscriber::new(store, metrics.clone());
    subscriber.observe(&records).await;
    subscriber.observe(&records).await;
    let text = metrics.render().unwrap();
    assert!(
        text.contains("made_protocol_claim_peak_width_sum{protocol=\"review\",state=\"REVIEW\"} 1"),
        "{text}"
    );
    assert!(text.contains("made_protocol_step_failure_total{protocol=\"review\",failure_kind=\"no_valid_proposal\",step=\"a\"} 1"), "{text}");
}
