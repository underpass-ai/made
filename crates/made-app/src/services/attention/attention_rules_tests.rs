use std::collections::BTreeMap;

use made_core::entities::ceremony_events::{CeremonyCompleted, CeremonyPaused, StepCompleted};
use made_core::entities::{AuditFact, AuditRecord, CeremonyEvent};
use made_core::ports::PositionedRecord;
use made_core::value_objects::{
    AttentionKind, Attributes, AuditActor, AuditActorKind, CeremonyId, CeremonyName,
    CeremonyVersion, EventId, GlobalPosition, LifecycleReason, RoleId, StateId, StepAttempt,
    StepId, StepIteration, StepOutput, StepResult,
};
use time::OffsetDateTime;

use super::*;

fn integrator() -> RoleId {
    RoleId::new("INTEGRATOR").unwrap()
}

fn positioned(event_id: &str, event: CeremonyEvent) -> PositionedRecord {
    let record = AuditRecord::first(AuditFact {
        event_id: EventId::new(event_id).unwrap(),
        event,
        ceremony_id: CeremonyId::new("loop-1").unwrap(),
        definition_name: CeremonyName::new("review").unwrap(),
        definition_version: CeremonyVersion::v1(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    })
    .unwrap();
    PositionedRecord {
        position: GlobalPosition::new(1).unwrap(),
        record,
    }
}

fn output(entries: &[(&str, serde_json::Value)]) -> StepOutput {
    StepOutput::new(
        Attributes::new(
            entries
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect::<BTreeMap<_, _>>(),
        )
        .unwrap(),
    )
}

fn completed(step: &str, out: StepOutput) -> CeremonyEvent {
    CeremonyEvent::StepCompleted(StepCompleted {
        step_id: StepId::new(step).unwrap(),
        state_visit: None,
        state_iteration: None,
        iteration: StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        result: StepResult::completed(out).unwrap(),
        next_iteration: None,
        finished_by: RoleId::new("REVIEWER").unwrap(),
        finished_at: OffsetDateTime::UNIX_EPOCH,
    })
}

#[test]
fn a_sealed_step_result_is_something_the_integrator_can_register() {
    let record = positioned("e1", completed("implement", output(&[])));

    let event = attention_for(&record, &integrator()).unwrap().unwrap();

    assert_eq!(event.kind(), AttentionKind::ResultAvailable);
    assert_eq!(event.acceptance(), ResultAcceptance::Accepted);
    assert_eq!(event.step_id().unwrap().as_str(), "implement");
}

#[test]
fn a_review_that_did_not_accept_the_work_reads_as_a_rejection() {
    let record = positioned(
        "e2",
        completed("review", output(&[("accepted", serde_json::json!(false))])),
    );

    let event = attention_for(&record, &integrator()).unwrap().unwrap();

    assert_eq!(event.kind(), AttentionKind::ReviewRejected);
    assert_eq!(
        event.acceptance(),
        ResultAcceptance::Accepted,
        "the rejection is itself a sealed fact; what was refused is the work, not the record"
    );
}

#[test]
fn a_review_that_accepted_the_work_is_an_ordinary_result() {
    let record = positioned(
        "e3",
        completed("review", output(&[("accepted", serde_json::json!(true))])),
    );

    let event = attention_for(&record, &integrator()).unwrap().unwrap();

    assert_eq!(event.kind(), AttentionKind::ResultAvailable);
}

#[test]
fn a_pause_is_a_state_of_the_loop_and_not_news() {
    let record = positioned(
        "e4",
        CeremonyEvent::CeremonyPaused(CeremonyPaused {
            reason: LifecycleReason::new("waiting on a person").unwrap(),
            paused_at: OffsetDateTime::UNIX_EPOCH,
        }),
    );

    assert!(attention_for(&record, &integrator()).unwrap().is_none());
}

#[test]
fn an_ended_ceremony_wakes_the_integrator_with_nothing_to_register() {
    let record = positioned(
        "e5",
        CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
            final_state: StateId::new("done").unwrap(),
            completed_at: OffsetDateTime::UNIX_EPOCH,
        }),
    );

    let event = attention_for(&record, &integrator()).unwrap().unwrap();

    assert_eq!(event.kind(), AttentionKind::CeremonyEnded);
    assert_eq!(event.acceptance(), ResultAcceptance::NotApplicable);
}

#[test]
fn the_same_record_always_derives_the_same_identity() {
    let record = positioned("e6", completed("implement", output(&[])));

    let first = attention_for(&record, &integrator()).unwrap().unwrap();
    let again = attention_for(&record, &integrator()).unwrap().unwrap();

    assert_eq!(
        first.id(),
        again.id(),
        "replaying the feed must not wake a host twice for one piece of news"
    );
}

#[test]
fn a_rejection_and_a_result_from_one_record_are_never_both_derived() {
    let rejected = positioned(
        "e7",
        completed("review", output(&[("accepted", serde_json::json!(false))])),
    );

    let event = attention_for(&rejected, &integrator()).unwrap().unwrap();

    assert_eq!(
        event.kind(),
        AttentionKind::ReviewRejected,
        "one record is one piece of news: a host told twice would act twice"
    );
}
