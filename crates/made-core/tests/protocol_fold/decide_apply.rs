//! For every command: `decide` yields the expected events, and
//! folding them leaves exactly the session the mutator leaves.

use made_core::entities::protocol_commands::{
    ApplyExecutionReceiptResult, ApplyStepResult, ApplyTransition, ApproveGuard, AssertReason,
    BindParticipant, CloseIntervention, DeferGuard, RequestIntervention, RespondToIntervention,
    RespondToInterventionWithEvidence, StartStep,
};
use made_core::entities::protocol_events::{
    EvidenceCollected, ExecutionReceiptLinked, HumanApprovalRecorded, HumanDeferralRecorded,
    InterventionClosed, InterventionRequested, InterventionResponded, ParticipantsBound,
    ProtocolCompleted, ProtocolInstanceStarted, ReasonAsserted, StepCompleted, StepFailed,
    StepStarted, TransitionApplied,
};
use made_core::entities::{
    ProtocolCommand, ProtocolDefinition, ProtocolEvent, ProtocolInstance, ProtocolIntervention,
    PublishedProtocolDefinition,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuditActorKind, ExecutionOperationId, ExecutionReceiptId, ExecutionReceiptLink,
    ExecutionReceiptLinkKind, MemoryConfidence, ProtocolContext, ProtocolGuardApproval,
    ProtocolGuardDeferral, ProtocolId, ProtocolInterventionKind, ProtocolInterventionProvenance,
    ProtocolInterventionResponse, ProtocolInterventionTarget, ProtocolParticipantBinding,
    ProtocolReason, ProtocolReasonKind, ProtocolRecordRef, ProtocolTransitionRecord,
    StateIteration, StateVisit, StepAttempt, StepClaimFence, StepErrorMessage, StepIteration,
    StepResult,
};

use super::fixture::{
    at, content, deferral, definition, evidence_pack, guard, item, lease, opened, readiness, role,
    specialty, state, step, trigger, OPENED_AT,
};

/// Decide `command` on `instance`, fold what it yields, run the
/// legacy `mutator` on a copy, and require both sessions to be equal.
/// Returns the events for the caller to pin.
fn decided_and_folded<T>(
    instance: &ProtocolInstance,
    definition: &ProtocolDefinition,
    command: &ProtocolCommand,
    mutator: impl FnOnce(&mut ProtocolInstance) -> Result<T, DomainError>,
) -> Vec<ProtocolEvent> {
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

fn with_plan_in_progress(definition: &ProtocolDefinition) -> ProtocolInstance {
    let mut instance = opened(definition);
    instance
        .start_step(definition, &step("plan"), lease("plan-1", at(1)), at(1))
        .unwrap();
    instance
}

fn with_open_item(definition: &ProtocolDefinition) -> ProtocolInstance {
    let mut instance = opened(definition);
    instance
        .request_intervention_as(
            definition,
            item("item-1"),
            role("facilitator"),
            ProtocolInterventionKind::Investigation,
            ProtocolInterventionTarget::roles([role("observer")]).unwrap(),
            content("Look at the queue."),
            at(1),
        )
        .unwrap();
    instance
}

fn with_answered_item(definition: &ProtocolDefinition) -> ProtocolInstance {
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
    let id = ProtocolId::new("protocol-fold").unwrap();

    let started = ProtocolInstance::decide_start(
        id.clone(),
        &definition,
        ProtocolContext::empty(),
        None,
        OPENED_AT,
    )
    .expect("required protocol inputs");
    let [opening] = started.as_slice() else {
        panic!("a session that recalls nothing opens with one event, got {started:?}");
    };
    let ProtocolEvent::ProtocolInstanceStarted(payload) = opening else {
        panic!("starting yields the opening event, got {opening:?}");
    };
    assert_eq!(
        payload,
        &ProtocolInstanceStarted {
            protocol_id: id.clone(),
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            initial_state: state("drafting"),
            step_ids: [step("plan"), step("check")].into_iter().collect(),
            context: ProtocolContext::empty(),
            bound_definition: None,
            lineage: None,
            succession: None,
            budget_account_id: None,
            protocol_deadline: None,
            state_deadline: None,
            created_at: OPENED_AT,
        }
    );
    assert_eq!(ProtocolInstance::from_started(payload), opened(&definition));
    assert_eq!(
        ProtocolInstance::rehydrate(&started).unwrap(),
        opened(&definition)
    );

    let published = PublishedProtocolDefinition::seal(definition.clone()).unwrap();
    let bound_opening = ProtocolInstance::decide_start_bound(
        id.clone(),
        &published,
        ProtocolContext::empty(),
        None,
        OPENED_AT,
    )
    .expect("required protocol inputs");
    let [ProtocolEvent::ProtocolInstanceStarted(bound)] = bound_opening.as_slice() else {
        panic!("starting bound yields the opening event, got {bound_opening:?}");
    };
    assert_eq!(bound.bound_definition, Some(published.digest()));
    assert_eq!(
        ProtocolInstance::from_started(bound),
        ProtocolInstance::start_bound(id, &published, ProtocolContext::empty(), OPENED_AT)
            .expect("required protocol inputs")
    );
}

#[test]
fn binding_a_participant() {
    let definition = definition();
    let instance = opened(&definition);

    let events = decided_and_folded(
        &instance,
        &definition,
        &ProtocolCommand::BindParticipant(BindParticipant {
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
        vec![ProtocolEvent::ParticipantsBound(ParticipantsBound {
            bindings: vec![ProtocolParticipantBinding::record(
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
        &ProtocolCommand::StartStep(StartStep {
            role_id: Some(role("facilitator")),
            step_id: step("plan"),
            lease: lease("plan-1", at(1)),
            now: at(1),
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
            budget_reservation_id: None,
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
        vec![ProtocolEvent::StepStarted(StepStarted {
            state_visit: Some(StateVisit::FIRST),
            step_id: step("plan"),
            state_iteration: Some(made_core::value_objects::StateIteration::FIRST),
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            lease: lease("plan-1", at(1)),
            started_by: role("facilitator"),
            role_from: None,
            sealed_role: None,
            deadline: None,
            budget_reservation_id: None,
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
        &ProtocolCommand::StartStep(StartStep {
            role_id: None,
            step_id: step("plan"),
            lease: lease("plan-1", at(1)),
            now: at(1),
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
            budget_reservation_id: None,
        }),
        |session| session.start_step(&definition, &step("plan"), lease("plan-1", at(1)), at(1)),
    );

    let [ProtocolEvent::StepStarted(started)] = events.as_slice() else {
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
        &ProtocolCommand::StartStep(StartStep {
            role_id: None,
            step_id: step("plan"),
            lease: lease("plan-2", at(7)),
            now: at(7),
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
            budget_reservation_id: None,
        }),
        |session| session.start_step(&definition, &step("plan"), lease("plan-2", at(7)), at(7)),
    );

    let [ProtocolEvent::StepStarted(started)] = events.as_slice() else {
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
        &ProtocolCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            claim_fence: instance.step_claim_fence(&step("plan")).unwrap(),
            result: result.clone(),
            now: at(2),
        }),
        |session| {
            session.apply_step_result(
                &definition,
                &step("plan"),
                session.step_claim_fence(&step("plan")).unwrap(),
                result.clone(),
                at(2),
            )
        },
    );

    assert_eq!(
        events,
        vec![ProtocolEvent::StepCompleted(StepCompleted {
            state_visit: Some(StateVisit::FIRST),
            step_id: step("plan"),
            state_iteration: Some(made_core::value_objects::StateIteration::FIRST),
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
fn a_receipt_link_and_its_step_result_are_one_atomic_decision() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);
    let record = instance.step_record(&step("plan")).unwrap();
    let claim_fence = instance.step_claim_fence(&step("plan")).unwrap();
    let operation_id = ExecutionOperationId::for_step(
        instance.id(),
        &step("plan"),
        record.state_visit(),
        record.state_iteration(),
        record.iteration(),
    );
    let link = ExecutionReceiptLink::new(
        ExecutionReceiptId::for_operation(&operation_id),
        operation_id.clone(),
        claim_fence.clone(),
        claim_fence.clone(),
        ExecutionReceiptLinkKind::Direct,
    )
    .unwrap();
    let result = StepResult::completed(readiness(true)).unwrap();
    let command = ProtocolCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
        step_id: step("plan"),
        claim_fence,
        receipt_link: link.clone(),
        result: result.clone(),
        now: at(2),
    });

    let events = instance.decide(&command, &definition).unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            ProtocolEvent::ExecutionReceiptLinked(ExecutionReceiptLinked { .. }),
            ProtocolEvent::StepCompleted(_)
        ]
    ));
    let mut folded = instance;
    for event in &events {
        folded.apply(event);
    }
    assert_eq!(folded.execution_receipt_link(&operation_id), Some(&link));
    assert_eq!(
        folded.step_record(&step("plan")).unwrap().output(),
        result.output()
    );
    assert!(folded.decide(&command, &definition).unwrap().is_empty());
}

#[test]
fn a_receipt_link_cannot_complete_a_different_claim_or_operation() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);
    let claim_fence = instance.step_claim_fence(&step("plan")).unwrap();
    let wrong_operation = ExecutionOperationId::for_step(
        instance.id(),
        &step("check"),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
    );
    let foreign_fence = StepClaimFence::new("9".repeat(64)).unwrap();
    let wrong_link = ExecutionReceiptLink::new(
        ExecutionReceiptId::for_operation(&wrong_operation),
        wrong_operation,
        foreign_fence,
        claim_fence.clone(),
        ExecutionReceiptLinkKind::Adopted,
    )
    .unwrap();
    let command = ProtocolCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
        step_id: step("plan"),
        claim_fence,
        receipt_link: wrong_link,
        result: StepResult::completed(readiness(true)).unwrap(),
        now: at(2),
    });

    assert!(instance.decide(&command, &definition).is_err());
    assert!(instance.execution_receipt_links().is_empty());
}

#[test]
fn a_final_result_carries_no_next_iteration() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);
    let result = StepResult::completed(readiness(true)).unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &ProtocolCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            claim_fence: instance.step_claim_fence(&step("plan")).unwrap(),
            result: result.clone(),
            now: at(2),
        }),
        |session| {
            session.apply_step_result(
                &definition,
                &step("plan"),
                session.step_claim_fence(&step("plan")).unwrap(),
                result.clone(),
                at(2),
            )
        },
    );

    let [ProtocolEvent::StepCompleted(completed)] = events.as_slice() else {
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
        &ProtocolCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            claim_fence: instance.step_claim_fence(&step("plan")).unwrap(),
            result: result.clone(),
            now: at(2),
        }),
        |session| {
            session.apply_step_result(
                &definition,
                &step("plan"),
                session.step_claim_fence(&step("plan")).unwrap(),
                result.clone(),
                at(2),
            )
        },
    );

    assert_eq!(
        events,
        vec![ProtocolEvent::StepFailed(StepFailed {
            state_visit: Some(StateVisit::FIRST),
            step_id: step("plan"),
            state_iteration: Some(made_core::value_objects::StateIteration::FIRST),
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
            instance.step_claim_fence(&step("plan")).unwrap(),
            StepResult::completed(readiness(true)).unwrap(),
            at(2),
        )
        .unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &ProtocolCommand::ApplyTransition(ApplyTransition {
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
        vec![ProtocolEvent::TransitionApplied(TransitionApplied {
            destination: Some(made_core::entities::protocol_events::StateVisitEntry {
                state_visit: StateVisit::new(2).unwrap(),
                step_ids: vec![step("check")],
                deadline: None,
            }),
            transition: ProtocolTransitionRecord::record_at(
                trigger("submit"),
                state("drafting"),
                StateIteration::FIRST,
                state("review"),
                Some(role("facilitator")),
                at(3),
            )
            .with_state_visit(StateVisit::FIRST),
        })]
    );
}

/// `abandon` leaves `drafting`, so it also waits on `plan` reaching
/// its stop condition: no move leaves a state whose repeating step
/// is unfinished.
#[test]
fn a_move_into_a_terminal_state_also_completes_the_protocol() {
    let definition = definition();
    let mut instance = with_plan_in_progress(&definition);
    instance
        .apply_step_result(
            &definition,
            &step("plan"),
            instance.step_claim_fence(&step("plan")).unwrap(),
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
        &ProtocolCommand::ApplyTransition(ApplyTransition {
            role_id: None,
            trigger: trigger("abandon"),
            now: at(2),
        }),
        |session| session.apply_transition(&definition, &trigger("abandon"), at(2)),
    );

    assert_eq!(
        events,
        vec![
            ProtocolEvent::TransitionApplied(TransitionApplied {
                destination: Some(made_core::entities::protocol_events::StateVisitEntry {
                    state_visit: StateVisit::new(2).unwrap(),
                    step_ids: vec![],
                    deadline: None,
                }),
                transition: ProtocolTransitionRecord::record_at(
                    trigger("abandon"),
                    state("drafting"),
                    StateIteration::FIRST,
                    state("done"),
                    None,
                    at(2),
                )
                .with_state_visit(StateVisit::FIRST),
            }),
            ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
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
        &ProtocolCommand::ApproveGuard(ApproveGuard {
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
        vec![ProtocolEvent::HumanApprovalRecorded(
            HumanApprovalRecorded {
                approval: ProtocolGuardApproval::record(
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
        &ProtocolCommand::DeferGuard(DeferGuard {
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
        vec![ProtocolEvent::HumanDeferralRecorded(
            HumanDeferralRecorded {
                deferral: ProtocolGuardDeferral::record(
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
    let target = ProtocolInterventionTarget::roles([role("observer")]).unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &ProtocolCommand::RequestIntervention(RequestIntervention {
            intent: None,
            delivery: None,
            supervisor: None,
            intervention_id: item("item-1"),
            role_id: role("facilitator"),
            kind: ProtocolInterventionKind::Investigation,
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
                ProtocolInterventionKind::Investigation,
                target.clone(),
                content("Look at the queue."),
                at(1),
            )
        },
    );

    assert_eq!(
        events,
        vec![ProtocolEvent::InterventionRequested(
            InterventionRequested {
                intervention: ProtocolIntervention::open(
                    item("item-1"),
                    ProtocolInterventionKind::Investigation,
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
    let provenance = ProtocolInterventionProvenance::selected_from(
        item("item-1"),
        role("observer"),
        role("observer"),
    );

    let events = decided_and_folded(
        &instance,
        &definition,
        &ProtocolCommand::RequestIntervention(RequestIntervention {
            intent: None,
            delivery: None,
            supervisor: None,
            intervention_id: item("item-2"),
            role_id: role("facilitator"),
            kind: ProtocolInterventionKind::Opinion,
            target: ProtocolInterventionTarget::table(),
            content: content("Say more."),
            provenance: Some(provenance.clone()),
            now: at(3),
        }),
        |session| {
            session.request_intervention_with_provenance_as(
                &definition,
                item("item-2"),
                role("facilitator"),
                ProtocolInterventionKind::Opinion,
                ProtocolInterventionTarget::table(),
                content("Say more."),
                Some(provenance.clone()),
                at(3),
            )
        },
    );

    let [ProtocolEvent::InterventionRequested(requested)] = events.as_slice() else {
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
        &ProtocolCommand::RespondToIntervention(RespondToIntervention {
            executor: None,
            delivery_id: None,
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
        vec![ProtocolEvent::InterventionResponded(
            InterventionResponded {
                intervention_id: item("item-1"),
                response: ProtocolInterventionResponse::new(
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
        &ProtocolCommand::RespondToInterventionWithEvidence(RespondToInterventionWithEvidence {
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
            ProtocolEvent::EvidenceCollected(EvidenceCollected {
                intervention_id: item("item-1"),
                source_id: pack.source_id().clone(),
                collected_by: role("observer"),
                evidence_pack: pack.clone(),
                collected_at: at(2),
            }),
            ProtocolEvent::InterventionResponded(InterventionResponded {
                intervention_id: item("item-1"),
                response: ProtocolInterventionResponse::from_evidence(
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
    let reason = ProtocolReason::new(
        ProtocolRecordRef::contribution(item("item-1"), 0),
        ProtocolRecordRef::agenda_item(item("item-1")),
        ProtocolReasonKind::ChosenBecause,
        "the queue was visibly empty",
        MemoryConfidence::High,
        Some(role("observer")),
        at(3),
    )
    .unwrap();

    let events = decided_and_folded(
        &instance,
        &definition,
        &ProtocolCommand::AssertReason(AssertReason {
            reason: reason.clone(),
        }),
        |session| {
            session.assert_reason_as(
                &definition,
                role("observer"),
                ProtocolRecordRef::contribution(item("item-1"), 0),
                ProtocolRecordRef::agenda_item(item("item-1")),
                ProtocolReasonKind::ChosenBecause,
                "the queue was visibly empty",
                MemoryConfidence::High,
                at(3),
            )
        },
    );

    assert_eq!(
        events,
        vec![ProtocolEvent::ReasonAsserted(ReasonAsserted { reason })]
    );
}

/// The engine's own reasons are recorded by the fold, never decided:
/// a reason with no seat is refused.
#[test]
fn a_reason_nobody_asserts_is_refused() {
    let definition = definition();
    let instance = with_answered_item(&definition);
    let reason = ProtocolReason::new(
        ProtocolRecordRef::contribution(item("item-1"), 0),
        ProtocolRecordRef::agenda_item(item("item-1")),
        ProtocolReasonKind::FollowsFrom,
        "the engine says so",
        MemoryConfidence::High,
        None,
        at(3),
    )
    .unwrap();

    let refused = instance.decide(
        &ProtocolCommand::AssertReason(AssertReason { reason }),
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
        &ProtocolCommand::CloseIntervention(CloseIntervention {
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
        vec![ProtocolEvent::InterventionClosed(InterventionClosed {
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
        &ProtocolCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            claim_fence: instance.step_claim_fence(&step("plan")).unwrap(),
            result: StepResult::completed(readiness(true)).unwrap(),
            now: at(2),
        }),
        &definition,
    );
    let refused = instance.decide(
        &ProtocolCommand::StartStep(StartStep {
            role_id: None,
            step_id: step("plan"),
            lease: lease("plan-1", at(2)),
            now: at(2),
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
            budget_reservation_id: None,
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
    let approval = ProtocolEvent::HumanApprovalRecorded(HumanApprovalRecorded {
        approval: ProtocolGuardApproval::record(
            guard("human_approved"),
            role("facilitator"),
            AuditActorKind::Human,
            at(1),
        ),
    });

    assert!(matches!(
        ProtocolInstance::rehydrate([&approval]),
        Err(DomainError::InvariantViolated { .. })
    ));
    assert!(matches!(
        ProtocolInstance::rehydrate(std::iter::empty()),
        Err(DomainError::InvariantViolated { .. })
    ));
}

#[test]
fn a_second_opening_leaves_the_session_untouched() {
    let definition = definition();
    let instance = with_plan_in_progress(&definition);
    let reopening = ProtocolInstance::decide_start(
        ProtocolId::new("someone-else").unwrap(),
        &definition,
        ProtocolContext::empty(),
        None,
        at(9),
    )
    .expect("required protocol inputs");

    let mut applied = instance.clone();
    for event in &reopening {
        applied.apply(event);
    }

    assert_eq!(applied, instance);
}
