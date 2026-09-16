//! For every command: `decide` yields the expected events, and
//! folding them leaves exactly the session the mutator leaves.

use made_core::entities::ceremony_commands::{
    ApplyStepResult, ApplyTransition, ApproveGuard, AssertReason, BindParticipant,
    CloseIntervention, DeferGuard, RequestIntervention, RespondToIntervention,
    RespondToInterventionWithEvidence, StartStep,
};
use made_core::entities::ceremony_events::{
    CeremonyCompleted, CeremonyInstanceStarted, EvidenceCollected, HumanApprovalRecorded,
    HumanDeferralRecorded, InterventionClosed, InterventionRequested, InterventionResponded,
    ParticipantsBound, ReasonAsserted, StepCompleted, StepFailed, StepStarted, TransitionApplied,
};
use made_core::entities::{
    CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance, CeremonyIntervention,
    PublishedCeremonyDefinition,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyGuardApproval, CeremonyGuardDeferral, CeremonyId,
    CeremonyInterventionKind, CeremonyInterventionProvenance, CeremonyInterventionResponse,
    CeremonyInterventionTarget, CeremonyParticipantBinding, CeremonyReason, CeremonyReasonKind,
    CeremonyRecordRef, CeremonyTransitionRecord, MemoryConfidence, StepAttempt, StepErrorMessage,
    StepIteration, StepResult,
};

use super::fixture::{
    at, content, deferral, definition, evidence_pack, guard, item, lease, opened, readiness, role,
    specialty, state, step, trigger, OPENED_AT,
};

/// Decide `command` on `instance`, fold what it yields, run the
/// legacy `mutator` on a copy, and require both sessions to be equal.
/// Returns the events for the caller to pin.
fn decided_and_folded<T>(
    instance: &CeremonyInstance,
    definition: &CeremonyDefinition,
    command: &CeremonyCommand,
    mutator: impl FnOnce(&mut CeremonyInstance) -> Result<T, DomainError>,
) -> Vec<CeremonyEvent> {
    let events = instance
        .decide(command, definition)
        .expect("the command is accepted");
    let mut by_fold = instance.clone();
    for event in &events {
        by_fold.apply(event);
    }
    let mut by_mutator = instance.clone();
    mutator(&mut by_mutator).expect("the mutator accepts what decide accepted");
    assert_eq!(by_fold, by_mutator, "fold and mutator disagree");
    events
}

fn with_plan_in_progress(definition: &CeremonyDefinition) -> CeremonyInstance {
    let mut instance = opened(definition);
    instance
        .start_step(definition, &step("plan"), lease("plan-1", at(1)), at(1))
        .unwrap();
    instance
}

fn with_open_item(definition: &CeremonyDefinition) -> CeremonyInstance {
    let mut instance = opened(definition);
    instance
        .request_intervention_as(
            definition,
            item("item-1"),
            role("facilitator"),
            CeremonyInterventionKind::Investigation,
            CeremonyInterventionTarget::roles([role("observer")]).unwrap(),
            content("Look at the queue."),
            at(1),
        )
        .unwrap();
    instance
}

fn with_answered_item(definition: &CeremonyDefinition) -> CeremonyInstance {
    let mut instance = with_open_item(definition);
    instance
        .respond_to_intervention_as(
            definition,
            &item("item-1"),
            role("observer"),
            content("It is empty."),
            at(2),
        )
        .unwrap();
    instance
}

#[test]
fn starting_is_the_fold_of_the_opening_event() {
    let definition = definition();
    let id = CeremonyId::new("ceremony-fold").unwrap();

    let started = CeremonyInstance::decide_start(
        id.clone(),
        &definition,
        CeremonyContext::empty(),
        OPENED_AT,
    );
    let CeremonyEvent::CeremonyInstanceStarted(payload) = &started else {
        panic!("starting yields the opening event, got {started:?}");
    };
    assert_eq!(
        payload,
        &CeremonyInstanceStarted {
            ceremony_id: id.clone(),
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            initial_state: state("drafting"),
            step_ids: [step("plan"), step("check")].into_iter().collect(),
            context: CeremonyContext::empty(),
            bound_definition: None,
            created_at: OPENED_AT,
        }
    );
    assert_eq!(CeremonyInstance::from_started(payload), opened(&definition));
    assert_eq!(
        CeremonyInstance::rehydrate([&started]).unwrap(),
        opened(&definition)
    );

    let published = PublishedCeremonyDefinition::seal(definition.clone()).unwrap();
    let CeremonyEvent::CeremonyInstanceStarted(bound) = CeremonyInstance::decide_start_bound(
        id.clone(),
        &published,
        CeremonyContext::empty(),
        OPENED_AT,
    ) else {
        panic!("starting bound yields the opening event");
    };
    assert_eq!(bound.bound_definition, Some(published.digest()));
    assert_eq!(
        CeremonyInstance::from_started(&bound),
        CeremonyInstance::start_bound(id, &published, CeremonyContext::empty(), OPENED_AT)
    );
}

#[test]
fn binding_a_participant() {
    let definition = definition();
    let instance = opened(&definition);

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::BindParticipant(BindParticipant {
            role_id: role("observer"),
            specialty: specialty("queues"),
            now: at(1),
        }),
        |session| {
            session.bind_participant(&definition, role("observer"), specialty("queues"), at(1))
        },
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::ParticipantsBound(ParticipantsBound {
            bindings: vec![CeremonyParticipantBinding::record(
                role("observer"),
                specialty("queues"),
                at(1)
            )],
        })]
    );
}

#[test]
fn starting_a_step_names_the_seat_that_took_it() {
    let definition = definition();
    let instance = opened(&definition);

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::StartStep(StartStep {
            role_id: Some(role("facilitator")),
            step_id: step("plan"),
            lease: lease("plan-1", at(1)),
            now: at(1),
        }),
        |session| {
            session.start_step_as(
                &definition,
                &role("facilitator"),
                &step("plan"),
                lease("plan-1", at(1)),
                at(1),
            )
        },
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::StepStarted(StepStarted {
            step_id: step("plan"),
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            lease: lease("plan-1", at(1)),
            started_by: role("facilitator"),
            started_at: at(1),
        })]
    );
}

/// A step the engine takes is still somebody's: the seat the
/// definition assigns to it.
#[test]
fn a_step_started_by_the_engine_names_the_definitions_seat() {
    let definition = definition();
    let instance = opened(&definition);

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::StartStep(StartStep {
            role_id: None,
            step_id: step("plan"),
            lease: lease("plan-1", at(1)),
            now: at(1),
        }),
        |session| session.start_step(&definition, &step("plan"), lease("plan-1", at(1)), at(1)),
    );

    let [CeremonyEvent::StepStarted(started)] = events.as_slice() else {
        panic!("expected one step start, got {events:?}");
    };
    assert_eq!(started.started_by, role("facilitator"));
}

/// A takeover after the lease expired is the next attempt.
#[test]
fn taking_over_an_expired_lease_is_the_next_attempt() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::StartStep(StartStep {
            role_id: None,
            step_id: step("plan"),
            lease: lease("plan-2", at(7)),
            now: at(7),
        }),
        |session| session.start_step(&definition, &step("plan"), lease("plan-2", at(7)), at(7)),
    );

    let [CeremonyEvent::StepStarted(started)] = events.as_slice() else {
        panic!("expected one step start, got {events:?}");
    };
    assert_eq!(started.attempt, StepAttempt::new(2).unwrap());
}

#[test]
fn a_result_that_reopens_the_step_carries_the_next_iteration() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);
    let result = StepResult::completed(readiness(false)).unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            result: result.clone(),
            now: at(2),
        }),
        |session| session.apply_step_result(&definition, &step("plan"), result.clone(), at(2)),
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::StepCompleted(StepCompleted {
            step_id: step("plan"),
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            result,
            next_iteration: Some(StepIteration::new(2).unwrap()),
            finished_by: role("facilitator"),
            finished_at: at(2),
        })]
    );
}

#[test]
fn a_final_result_carries_no_next_iteration() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);
    let result = StepResult::completed(readiness(true)).unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            result: result.clone(),
            now: at(2),
        }),
        |session| session.apply_step_result(&definition, &step("plan"), result.clone(), at(2)),
    );

    let [CeremonyEvent::StepCompleted(completed)] = events.as_slice() else {
        panic!("expected one completion, got {events:?}");
    };
    assert_eq!(completed.next_iteration, None);
}

#[test]
fn a_failure_is_its_own_event() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);
    let result = StepResult::failed(StepErrorMessage::new("no plan").unwrap()).unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            result: result.clone(),
            now: at(2),
        }),
        |session| session.apply_step_result(&definition, &step("plan"), result.clone(), at(2)),
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::StepFailed(StepFailed {
            step_id: step("plan"),
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            result,
            finished_by: role("facilitator"),
            finished_at: at(2),
        })]
    );
}

#[test]
fn a_move_into_an_intermediate_state_is_one_event() {
    let definition = definition();
    let mut instance = with_plan_in_progress(&definition);
    instance
        .apply_step_result(
            &definition,
            &step("plan"),
            StepResult::completed(readiness(true)).unwrap(),
            at(2),
        )
        .unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::ApplyTransition(ApplyTransition {
            role_id: Some(role("facilitator")),
            trigger: trigger("submit"),
            now: at(3),
        }),
        |session| {
            session.apply_transition_as(
                &definition,
                &role("facilitator"),
                &trigger("submit"),
                at(3),
            )
        },
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::TransitionApplied(TransitionApplied {
            transition: CeremonyTransitionRecord::record(
                trigger("submit"),
                state("drafting"),
                state("review"),
                Some(role("facilitator")),
                at(3),
            ),
        })]
    );
}

/// `abandon` leaves `drafting`, so it also waits on `plan` reaching
/// its stop condition: no move leaves a state whose repeating step
/// is unfinished.
#[test]
fn a_move_into_a_terminal_state_also_completes_the_ceremony() {
    let definition = definition();
    let mut instance = with_plan_in_progress(&definition);
    instance
        .apply_step_result(
            &definition,
            &step("plan"),
            StepResult::completed(readiness(true)).unwrap(),
            at(1),
        )
        .unwrap();
    instance
        .approve_guard(
            &definition,
            &guard("human_approved"),
            role("facilitator"),
            AuditActorKind::Human,
            at(1),
        )
        .unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::ApplyTransition(ApplyTransition {
            role_id: None,
            trigger: trigger("abandon"),
            now: at(2),
        }),
        |session| session.apply_transition(&definition, &trigger("abandon"), at(2)),
    );

    assert_eq!(
        events,
        vec![
            CeremonyEvent::TransitionApplied(TransitionApplied {
                transition: CeremonyTransitionRecord::record(
                    trigger("abandon"),
                    state("drafting"),
                    state("done"),
                    None,
                    at(2),
                ),
            }),
            CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
                final_state: state("done"),
                completed_at: at(2),
            }),
        ]
    );
}

#[test]
fn approving_a_guard() {
    let definition = definition();
    let instance = opened(&definition);

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::ApproveGuard(ApproveGuard {
            guard_name: guard("human_approved"),
            approved_by: role("facilitator"),
            approved_by_kind: AuditActorKind::Human,
            now: at(1),
        }),
        |session| {
            session.approve_guard(
                &definition,
                &guard("human_approved"),
                role("facilitator"),
                AuditActorKind::Human,
                at(1),
            )
        },
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::HumanApprovalRecorded(
            HumanApprovalRecorded {
                approval: CeremonyGuardApproval::record(
                    guard("human_approved"),
                    role("facilitator"),
                    AuditActorKind::Human,
                    at(1),
                ),
            }
        )]
    );
}

#[test]
fn deferring_a_guard() {
    let definition = definition();
    let instance = opened(&definition);

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::DeferGuard(DeferGuard {
            guard_name: guard("human_approved"),
            content: deferral(),
            deferred_by: role("observer"),
            deferred_by_kind: AuditActorKind::Human,
            now: at(1),
        }),
        |session| {
            session.defer_guard(
                &definition,
                guard("human_approved"),
                deferral(),
                role("observer"),
                AuditActorKind::Human,
                at(1),
            )
        },
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::HumanDeferralRecorded(
            HumanDeferralRecorded {
                deferral: CeremonyGuardDeferral::record(
                    guard("human_approved"),
                    role("observer"),
                    AuditActorKind::Human,
                    deferral(),
                    at(1),
                ),
            }
        )]
    );
}

#[test]
fn requesting_an_intervention() {
    let definition = definition();
    let instance = opened(&definition);
    let target = CeremonyInterventionTarget::roles([role("observer")]).unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::RequestIntervention(RequestIntervention {
            intervention_id: item("item-1"),
            role_id: role("facilitator"),
            kind: CeremonyInterventionKind::Investigation,
            target: target.clone(),
            content: content("Look at the queue."),
            provenance: None,
            now: at(1),
        }),
        |session| {
            session.request_intervention_as(
                &definition,
                item("item-1"),
                role("facilitator"),
                CeremonyInterventionKind::Investigation,
                target.clone(),
                content("Look at the queue."),
                at(1),
            )
        },
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::InterventionRequested(
            InterventionRequested {
                intervention: CeremonyIntervention::open(
                    item("item-1"),
                    CeremonyInterventionKind::Investigation,
                    role("facilitator"),
                    target,
                    content("Look at the queue."),
                    at(1),
                ),
            }
        )]
    );
}

#[test]
fn requesting_an_intervention_selected_out_of_a_response() {
    let definition = definition();
    let instance = with_answered_item(&definition);
    let provenance = CeremonyInterventionProvenance::selected_from(
        item("item-1"),
        role("observer"),
        role("observer"),
    );

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::RequestIntervention(RequestIntervention {
            intervention_id: item("item-2"),
            role_id: role("facilitator"),
            kind: CeremonyInterventionKind::Opinion,
            target: CeremonyInterventionTarget::table(),
            content: content("Say more."),
            provenance: Some(provenance.clone()),
            now: at(3),
        }),
        |session| {
            session.request_intervention_with_provenance_as(
                &definition,
                item("item-2"),
                role("facilitator"),
                CeremonyInterventionKind::Opinion,
                CeremonyInterventionTarget::table(),
                content("Say more."),
                Some(provenance.clone()),
                at(3),
            )
        },
    );

    let [CeremonyEvent::InterventionRequested(requested)] = events.as_slice() else {
        panic!("expected one request, got {events:?}");
    };
    assert_eq!(requested.intervention.provenance(), Some(&provenance));
}

#[test]
fn responding_to_an_intervention() {
    let definition = definition();
    let instance = with_open_item(&definition);

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::RespondToIntervention(RespondToIntervention {
            intervention_id: item("item-1"),
            role_id: role("observer"),
            content: content("It is empty."),
            now: at(2),
        }),
        |session| {
            session.respond_to_intervention_as(
                &definition,
                &item("item-1"),
                role("observer"),
                content("It is empty."),
                at(2),
            )
        },
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::InterventionResponded(
            InterventionResponded {
                intervention_id: item("item-1"),
                response: CeremonyInterventionResponse::new(
                    role("observer"),
                    content("It is empty."),
                    at(2)
                ),
            }
        )]
    );
}

#[test]
fn responding_out_of_a_source_is_a_receipt_and_an_answer() {
    let definition = definition();
    let instance = with_open_item(&definition);
    let pack = evidence_pack("wiki");

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::RespondToInterventionWithEvidence(RespondToInterventionWithEvidence {
            intervention_id: item("item-1"),
            role_id: role("observer"),
            evidence_pack: pack.clone(),
            now: at(2),
        }),
        |session| {
            session.respond_to_intervention_with_evidence_as(
                &definition,
                &item("item-1"),
                role("observer"),
                pack.clone(),
                at(2),
            )
        },
    );

    assert_eq!(
        events,
        vec![
            CeremonyEvent::EvidenceCollected(EvidenceCollected {
                intervention_id: item("item-1"),
                source_id: pack.source_id().clone(),
                collected_by: role("observer"),
                evidence_pack: pack.clone(),
                collected_at: at(2),
            }),
            CeremonyEvent::InterventionResponded(InterventionResponded {
                intervention_id: item("item-1"),
                response: CeremonyInterventionResponse::from_evidence(
                    role("observer"),
                    pack,
                    at(2)
                )
                .unwrap(),
            }),
        ]
    );
}

#[test]
fn asserting_a_reason() {
    let definition = definition();
    let instance = with_answered_item(&definition);
    let reason = CeremonyReason::new(
        CeremonyRecordRef::contribution(item("item-1"), 0),
        CeremonyRecordRef::agenda_item(item("item-1")),
        CeremonyReasonKind::ChosenBecause,
        "the queue was visibly empty",
        MemoryConfidence::High,
        Some(role("observer")),
        at(3),
    )
    .unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::AssertReason(AssertReason {
            reason: reason.clone(),
        }),
        |session| {
            session.assert_reason_as(
                &definition,
                role("observer"),
                CeremonyRecordRef::contribution(item("item-1"), 0),
                CeremonyRecordRef::agenda_item(item("item-1")),
                CeremonyReasonKind::ChosenBecause,
                "the queue was visibly empty",
                MemoryConfidence::High,
                at(3),
            )
        },
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::ReasonAsserted(ReasonAsserted { reason })]
    );
}

/// The engine's own reasons are recorded by the fold, never decided:
/// a reason with no seat is refused.
#[test]
fn a_reason_nobody_asserts_is_refused() {
    let definition = definition();
    let instance = with_answered_item(&definition);
    let reason = CeremonyReason::new(
        CeremonyRecordRef::contribution(item("item-1"), 0),
        CeremonyRecordRef::agenda_item(item("item-1")),
        CeremonyReasonKind::FollowsFrom,
        "the engine says so",
        MemoryConfidence::High,
        None,
        at(3),
    )
    .unwrap();

    let refused = instance.decide(
        &CeremonyCommand::AssertReason(AssertReason { reason }),
        &definition,
    );

    assert!(matches!(
        refused,
        Err(DomainError::InvariantViolated { .. })
    ));
}

#[test]
fn closing_an_intervention() {
    let definition = definition();
    let instance = with_answered_item(&definition);

    let events = decided_and_folded(
        &instance,
        &definition,
        &CeremonyCommand::CloseIntervention(CloseIntervention {
            intervention_id: item("item-1"),
            role_id: role("facilitator"),
            now: at(3),
        }),
        |session| {
            session.close_intervention_as(&definition, &item("item-1"), &role("facilitator"), at(3))
        },
    );

    assert_eq!(
        events,
        vec![CeremonyEvent::InterventionClosed(InterventionClosed {
            intervention_id: item("item-1"),
            closed_by: role("facilitator"),
            closed_at: at(3),
        })]
    );
}

/// Deciding writes nothing, whichever way it goes.
#[test]
fn deciding_leaves_the_session_untouched() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);
    let before = instance.clone();

    let accepted = instance.decide(
        &CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            result: StepResult::completed(readiness(true)).unwrap(),
            now: at(2),
        }),
        &definition,
    );
    let refused = instance.decide(
        &CeremonyCommand::StartStep(StartStep {
            role_id: None,
            step_id: step("plan"),
            lease: lease("plan-1", at(2)),
            now: at(2),
        }),
        &definition,
    );

    assert!(accepted.is_ok());
    assert_eq!(
        refused,
        Err(DomainError::InvariantViolated {
            reason: "step lease is still active"
        })
    );
    assert_eq!(instance, before);
}

#[test]
fn a_stream_that_does_not_open_with_a_start_is_refused() {
    let approval = CeremonyEvent::HumanApprovalRecorded(HumanApprovalRecorded {
        approval: CeremonyGuardApproval::record(
            guard("human_approved"),
            role("facilitator"),
            AuditActorKind::Human,
            at(1),
        ),
    });

    assert!(matches!(
        CeremonyInstance::rehydrate([&approval]),
        Err(DomainError::InvariantViolated { .. })
    ));
    assert!(matches!(
        CeremonyInstance::rehydrate(std::iter::empty()),
        Err(DomainError::InvariantViolated { .. })
    ));
}

#[test]
fn a_second_opening_leaves_the_session_untouched() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);
    let reopening = CeremonyInstance::decide_start(
        CeremonyId::new("someone-else").unwrap(),
        &definition,
        CeremonyContext::empty(),
        at(9),
    );

    let mut applied = instance.clone();
    applied.apply(&reopening);

    assert_eq!(applied, instance);
}
