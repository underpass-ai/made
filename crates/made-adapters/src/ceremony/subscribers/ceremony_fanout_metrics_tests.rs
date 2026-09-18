use std::collections::BTreeSet;
use std::sync::Arc;

use made_core::entities::ceremony_events::{
    CeremonyCompleted, CeremonyInstanceStarted, StateIterationStarted, StepCompleted, StepFailed,
    StepStarted,
};
use made_core::entities::{AuditFact, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::{CeremonyEventStorePort, CeremonyEventSubscriberPort};
use made_core::value_objects::*;
use time::{Duration, OffsetDateTime};

use super::CeremonyFanoutMetricsSubscriber;
use crate::memory::InMemoryCeremonyEventStore;
use crate::metrics::PrometheusMetricsRecorder;

fn start() -> CeremonyEvent {
    CeremonyEvent::CeremonyInstanceStarted(CeremonyInstanceStarted {
        ceremony_id: CeremonyId::new("fanout").unwrap(),
        definition_name: CeremonyName::new("review").unwrap(),
        definition_version: CeremonyVersion::v1(),
        initial_state: StateId::new("REVIEW").unwrap(),
        step_ids: BTreeSet::from([StepId::new("a").unwrap(), StepId::new("b").unwrap()]),
        context: CeremonyContext::empty(),
        bound_definition: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    })
}

fn claim(step: &str, at: i64, iteration: StateIteration) -> CeremonyEvent {
    let now = OffsetDateTime::UNIX_EPOCH + Duration::seconds(at);
    CeremonyEvent::StepStarted(StepStarted {
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
        started_at: now,
    })
}

fn finish(step: &str, at: i64, iteration: StateIteration) -> CeremonyEvent {
    CeremonyEvent::StepCompleted(StepCompleted {
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

fn done(at: i64) -> CeremonyEvent {
    CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
        final_state: StateId::new("DONE").unwrap(),
        completed_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(at),
    })
}

async fn store(events: Vec<CeremonyEvent>) -> Arc<InMemoryCeremonyEventStore> {
    let store = Arc::new(InMemoryCeremonyEventStore::new());
    let facts = events
        .into_iter()
        .enumerate()
        .map(|(index, event)| AuditFact {
            event_id: EventId::new(format!("event-{index}")).unwrap(),
            event,
            ceremony_id: CeremonyId::new("fanout").unwrap(),
            definition_name: CeremonyName::new("review").unwrap(),
            definition_version: CeremonyVersion::v1(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        })
        .collect();
    store
        .append(
            &CeremonyId::new("fanout").unwrap(),
            StreamVersion::EMPTY,
            facts,
        )
        .await
        .unwrap();
    store
}

#[tokio::test]
async fn out_of_order_and_duplicate_notifications_project_one_peak_and_typed_failure() {
    let failed = CeremonyEvent::StepFailed(StepFailed {
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
        .read_all(GlobalPosition::FIRST, CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    let metrics = Arc::new(PrometheusMetricsRecorder::new().unwrap());
    let subscriber = CeremonyFanoutMetricsSubscriber::new(store, metrics.clone());
    subscriber.observe(&records[records.len() - 1..]).await;
    subscriber.observe(&records[..2]).await;
    subscriber.observe(&records).await;
    let text = metrics.render().unwrap();
    assert!(
        text.contains("made_ceremony_claim_peak_width_sum{ceremony=\"review\",state=\"REVIEW\"} 2"),
        "{text}"
    );
    assert!(
        text.contains(
            "made_ceremony_claim_peak_width_count{ceremony=\"review\",state=\"REVIEW\"} 1"
        ),
        "{text}"
    );
    assert!(text.contains("made_ceremony_sibling_failure_total{ceremony=\"review\",failure_kind=\"no_valid_proposal\",step=\"a\"} 1"), "{text}");
}

#[tokio::test]
async fn expiry_and_state_repeat_do_not_inflate_peak() {
    let second = StateIteration::new(2).unwrap();
    let repeat = CeremonyEvent::StateIterationStarted(StateIterationStarted {
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
        .read_all(GlobalPosition::FIRST, CeremonyEventPageLimit::DEFAULT)
        .await
        .unwrap();
    let metrics = Arc::new(PrometheusMetricsRecorder::new().unwrap());
    let subscriber = CeremonyFanoutMetricsSubscriber::new(store, metrics.clone());
    subscriber.observe(&records).await;
    let text = metrics.render().unwrap();
    assert!(
        text.contains("made_ceremony_claim_peak_width_sum{ceremony=\"review\",state=\"REVIEW\"} 3"),
        "{text}"
    );
    assert!(
        text.contains(
            "made_ceremony_claim_peak_width_count{ceremony=\"review\",state=\"REVIEW\"} 2"
        ),
        "{text}"
    );
    assert!(!text.contains("made_ceremony_sibling_failure_total{"));
}
