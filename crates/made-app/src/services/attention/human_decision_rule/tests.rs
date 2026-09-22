use made_core::entities::ceremony_events::TransitionApplied;
use made_core::entities::{AuditFact, AuditRecord, CeremonyDefinition, CeremonyEvent};
use made_core::ports::PositionedRecord;
use made_core::value_objects::{
    AuditActor, AuditActorKind, CeremonyGuard, CeremonyId, CeremonyName, CeremonyState,
    CeremonyStateKind, CeremonyTransition, CeremonyTransitionRecord, CeremonyVersion, EventId,
    GlobalPosition, GuardCondition, RoleId, TransitionTrigger,
};
use time::OffsetDateTime;

use super::*;

fn state(id: &str, kind: CeremonyStateKind) -> CeremonyState {
    CeremonyState::new(StateId::new(id).unwrap(), kind)
}

fn guard_name(name: &str) -> GuardName {
    GuardName::new(name).unwrap()
}

/// `review` leaves on an automated guard; `human_approval` leaves only
/// when a person says so, and `signed_off` is the second person.
fn definition(second_approver: bool) -> CeremonyDefinition {
    let mut guards = vec![
        CeremonyGuard::new(guard_name("reviewed"), GuardCondition::AllStepsCompleted),
        CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval),
    ];
    let mut transitions = vec![
        CeremonyTransition::new(
            StateId::new("review").unwrap(),
            StateId::new("human_approval").unwrap(),
            TransitionTrigger::new("accept").unwrap(),
            [guard_name("reviewed")],
        )
        .unwrap(),
        CeremonyTransition::new(
            StateId::new("human_approval").unwrap(),
            StateId::new("done").unwrap(),
            TransitionTrigger::new("approve").unwrap(),
            [guard_name("human_approved")],
        )
        .unwrap(),
    ];
    if second_approver {
        guards.push(CeremonyGuard::new(
            guard_name("countersigned"),
            GuardCondition::HumanApproval,
        ));
        transitions.push(
            CeremonyTransition::new(
                StateId::new("human_approval").unwrap(),
                StateId::new("done").unwrap(),
                TransitionTrigger::new("countersign").unwrap(),
                [guard_name("countersigned")],
            )
            .unwrap(),
        );
    }
    CeremonyDefinition::new(
        CeremonyName::new("loop").unwrap(),
        CeremonyVersion::v1(),
        None,
        [],
        [],
        [
            state("review", CeremonyStateKind::Initial),
            state("human_approval", CeremonyStateKind::Intermediate),
            state("done", CeremonyStateKind::Terminal),
        ],
        transitions,
        [],
        guards,
        [],
    )
    .unwrap()
}

fn transition_into(to: &str, event_id: &str, position: u64) -> PositionedRecord {
    let record = AuditRecord::first(AuditFact {
        event_id: EventId::new(event_id).unwrap(),
        event: CeremonyEvent::TransitionApplied(TransitionApplied {
            transition: CeremonyTransitionRecord::record(
                TransitionTrigger::new("accept").unwrap(),
                StateId::new("review").unwrap(),
                StateId::new(to).unwrap(),
                Some(RoleId::new("INTEGRATOR").unwrap()),
                OffsetDateTime::UNIX_EPOCH,
            ),
            destination: None,
        }),
        ceremony_id: CeremonyId::new("loop-1").unwrap(),
        definition_name: CeremonyName::new("loop").unwrap(),
        definition_version: CeremonyVersion::v1(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    })
    .unwrap();
    PositionedRecord {
        position: GlobalPosition::new(position).unwrap(),
        record,
    }
}

#[test]
fn entering_a_state_only_a_person_can_leave_is_news() {
    let events = human_decisions_requested(
        &transition_into("human_approval", "moved-1", 7),
        &definition(false),
    )
    .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind(), AttentionKind::HumanDecisionRequested);
    assert!(
        events[0].reason().as_str().contains("human_approved"),
        "the guard has to be named: {}",
        events[0].reason()
    );
    assert!(
        events[0]
            .reason()
            .as_str()
            .contains("do not answer it yourself"),
        "the loop is told what it may not do: {}",
        events[0].reason()
    );
}

#[test]
fn a_state_nobody_has_to_answer_for_is_not_news() {
    assert!(
        human_decisions_requested(&transition_into("done", "moved-2", 8), &definition(false))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn two_people_owed_an_answer_are_two_pieces_of_news() {
    let events = human_decisions_requested(
        &transition_into("human_approval", "moved-3", 9),
        &definition(true),
    )
    .unwrap();

    assert_eq!(events.len(), 2);
    let ids = events
        .iter()
        .map(|event| event.id().as_str().to_owned())
        .collect::<Vec<_>>();
    assert_ne!(ids[0], ids[1], "one identity for both would be closed once");
    assert!(ids.iter().all(|id| id.contains(":guard:")));
}

#[test]
fn the_same_visit_always_derives_the_same_identity() {
    let first = human_decisions_requested(
        &transition_into("human_approval", "moved-4", 3),
        &definition(false),
    )
    .unwrap();
    let again = human_decisions_requested(
        &transition_into("human_approval", "moved-4", 3),
        &definition(false),
    )
    .unwrap();

    assert_eq!(first[0].id(), again[0].id());
    assert_eq!(
        first[0].id().source_position(first[0].ceremony_id()),
        Some(GlobalPosition::new(3).unwrap()),
        "the identity still says where to read the record"
    );
}

#[test]
fn a_record_that_is_not_a_transition_says_nothing() {
    let record = AuditRecord::first(AuditFact {
        event_id: EventId::new("started-1").unwrap(),
        event: CeremonyEvent::CeremonyPaused(
            made_core::entities::ceremony_events::CeremonyPaused {
                reason: made_core::value_objects::LifecycleReason::new("a person stepped away")
                    .unwrap(),
                paused_at: OffsetDateTime::UNIX_EPOCH,
            },
        ),
        ceremony_id: CeremonyId::new("loop-1").unwrap(),
        definition_name: CeremonyName::new("loop").unwrap(),
        definition_version: CeremonyVersion::v1(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    })
    .unwrap();
    let positioned = PositionedRecord {
        position: GlobalPosition::new(1).unwrap(),
        record,
    };

    assert!(human_decisions_requested(&positioned, &definition(false))
        .unwrap()
        .is_empty());
}
