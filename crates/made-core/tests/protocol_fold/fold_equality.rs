//! After a whole lifecycle of mutations, the session is the fold of
//! the events those mutations decided — and the mutators and the
//! decide-then-apply path agree at every step.

use made_core::entities::protocol_commands::{
    ApplyStepResult, ApplyTransition, ApproveGuard, AssertReason, BindParticipant,
    CloseIntervention, DeferGuard, RequestIntervention, RespondToIntervention,
    RespondToInterventionWithEvidence, StartStep,
};
use made_core::entities::{ProtocolCommand, ProtocolDefinition, ProtocolEvent, ProtocolInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuditActorKind, AuditEventType, MemoryConfidence, ProtocolContext, ProtocolId,
    ProtocolInterventionKind, ProtocolInterventionTarget, ProtocolReason, ProtocolReasonKind,
    ProtocolRecordRef, StepErrorMessage, StepResult,
};

use super::fixture::{
    at, content, deferral, definition, evidence_pack, guard, item, lease, readiness, role,
    specialty, step, trigger, OPENED_AT,
};

/// Run the legacy mutator that `command` replaces.
pub(crate) fn mutate(
    instance: &mut ProtocolInstance,
    definition: &ProtocolDefinition,
    command: &ProtocolCommand,
) -> Result<(), DomainError> {
    match command {
        ProtocolCommand::BindParticipant(c) => {
            instance.bind_participant(definition, c.role_id.clone(), c.specialty.clone(), c.now)
        }
        ProtocolCommand::StartStep(c) => start_step(instance, definition, c),
        ProtocolCommand::ApplyStepResult(c) => instance.apply_step_result(
            definition,
            &c.step_id,
            c.claim_fence.clone(),
            c.result.clone(),
            c.now,
        ),
        ProtocolCommand::ApplyTransition(c) => apply_transition(instance, definition, c),
        ProtocolCommand::ApproveGuard(c) => instance.approve_guard(
            definition,
            &c.guard_name,
            c.approved_by.clone(),
            c.approved_by_kind,
            c.now,
        ),
        ProtocolCommand::DeferGuard(c) => instance.defer_guard(
            definition,
            c.guard_name.clone(),
            c.content.clone(),
            c.deferred_by.clone(),
            c.deferred_by_kind,
            c.now,
        ),
        ProtocolCommand::RequestIntervention(c) => request_intervention(instance, definition, c),
        ProtocolCommand::RespondToIntervention(c) => instance.respond_to_intervention_as(
            definition,
            &c.intervention_id,
            c.role_id.clone(),
            c.content.clone(),
            c.now,
        ),
        ProtocolCommand::RespondToInterventionWithEvidence(c) => instance
            .respond_to_intervention_with_evidence_as(
                definition,
                &c.intervention_id,
                c.role_id.clone(),
                c.evidence_pack.clone(),
                c.now,
            ),
        ProtocolCommand::AssertReason(c) => assert_reason(instance, definition, &c.reason),
        ProtocolCommand::CloseIntervention(c) => {
            instance.close_intervention_as(definition, &c.intervention_id, &c.role_id, c.now)
        }
        ProtocolCommand::RecordHostHandoff(_)
        | ProtocolCommand::AcknowledgeInterventionDelivery(_)
        | ProtocolCommand::RenewStepLease(_)
        | ProtocolCommand::PlanProtocolChildren(_)
        | ProtocolCommand::PlanSuccessor(_)
        | ProtocolCommand::ApplyExecutionReceiptResult(_)
        | ProtocolCommand::AdoptChildSpawnPlan(_)
        | ProtocolCommand::AcceptChildCompletion(_)
        | ProtocolCommand::PauseProtocol(_)
        | ProtocolCommand::ResumeProtocol(_)
        | ProtocolCommand::CancelProtocol(_)
        | ProtocolCommand::EnforceProtocolDeadlines(_) => {
            for event in instance.decide(command, definition)? {
                instance.apply(&event);
            }
            Ok(())
        }
    }
}

fn start_step(
    instance: &mut ProtocolInstance,
    definition: &ProtocolDefinition,
    command: &StartStep,
) -> Result<(), DomainError> {
    let lease = command.lease.clone();
    match command.role_id.as_ref() {
        Some(role_id) => {
            instance.start_step_as(definition, role_id, &command.step_id, lease, command.now)
        }
        None => instance.start_step(definition, &command.step_id, lease, command.now),
    }
    .map(|_| ())
}

fn apply_transition(
    instance: &mut ProtocolInstance,
    definition: &ProtocolDefinition,
    command: &ApplyTransition,
) -> Result<(), DomainError> {
    match command.role_id.as_ref() {
        Some(role_id) => {
            instance.apply_transition_as(definition, role_id, &command.trigger, command.now)
        }
        None => instance.apply_transition(definition, &command.trigger, command.now),
    }
    .map(|_| ())
}

fn request_intervention(
    instance: &mut ProtocolInstance,
    definition: &ProtocolDefinition,
    command: &RequestIntervention,
) -> Result<(), DomainError> {
    instance.request_intervention_with_provenance_as(
        definition,
        command.intervention_id.clone(),
        command.role_id.clone(),
        command.kind,
        command.target.clone(),
        command.content.clone(),
        command.provenance.clone(),
        command.now,
    )
}

fn assert_reason(
    instance: &mut ProtocolInstance,
    definition: &ProtocolDefinition,
    reason: &ProtocolReason,
) -> Result<(), DomainError> {
    let role_id = reason
        .asserted_by()
        .cloned()
        .ok_or(DomainError::InvariantViolated {
            reason: "the mutator always names a seat",
        })?;
    instance.assert_reason_as(
        definition,
        role_id,
        reason.from().clone(),
        reason.to().clone(),
        reason.kind(),
        reason.why(),
        reason.confidence(),
        reason.asserted_at(),
    )
}

/// A whole session: seating, a repeated step, two interventions
/// answered two ways, a reason, a deferral then an approval, a
/// move, a retried step, and the move that ends it.
fn lifecycle() -> Vec<ProtocolCommand> {
    let mut commands = drafting();
    commands.extend(review());
    commands
}

/// Everything that happens in `drafting`.
fn drafting() -> Vec<ProtocolCommand> {
    let observer = ProtocolInterventionTarget::roles([role("observer")]).unwrap();
    vec![
        ProtocolCommand::BindParticipant(BindParticipant {
            role_id: role("observer"),
            specialty: specialty("queues"),
            now: at(1),
        }),
        ProtocolCommand::StartStep(StartStep {
            role_id: Some(role("facilitator")),
            step_id: step("plan"),
            lease: lease("plan-1", at(2)),
            now: at(2),
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
            budget_reservation_id: None,
        }),
        ProtocolCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            claim_fence: made_core::value_objects::StepClaimFence::new("0".repeat(64)).unwrap(),
            result: StepResult::completed(readiness(false)).unwrap(),
            now: at(3),
        }),
        ProtocolCommand::StartStep(StartStep {
            role_id: None,
            step_id: step("plan"),
            lease: lease("plan-2", at(4)),
            now: at(4),
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
            budget_reservation_id: None,
        }),
        ProtocolCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            claim_fence: made_core::value_objects::StepClaimFence::new("0".repeat(64)).unwrap(),
            result: StepResult::completed(readiness(true)).unwrap(),
            now: at(5),
        }),
        ProtocolCommand::RequestIntervention(RequestIntervention {
            intent: None,
            delivery: None,
            supervisor: None,
            intervention_id: item("item-1"),
            role_id: role("facilitator"),
            kind: ProtocolInterventionKind::Investigation,
            target: observer.clone(),
            content: content("Look at the queue."),
            provenance: None,
            now: at(6),
        }),
        ProtocolCommand::RespondToIntervention(RespondToIntervention {
            executor: None,
            delivery_id: None,
            intervention_id: item("item-1"),
            role_id: role("observer"),
            content: content("It is empty."),
            now: at(7),
        }),
        ProtocolCommand::RequestIntervention(RequestIntervention {
            intent: None,
            delivery: None,
            supervisor: None,
            intervention_id: item("item-2"),
            role_id: role("facilitator"),
            kind: ProtocolInterventionKind::Investigation,
            target: observer,
            content: content("What does the wiki say?"),
            provenance: None,
            now: at(8),
        }),
        ProtocolCommand::RespondToInterventionWithEvidence(RespondToInterventionWithEvidence {
            intervention_id: item("item-2"),
            role_id: role("observer"),
            evidence_pack: evidence_pack("wiki"),
            now: at(9),
        }),
        ProtocolCommand::AssertReason(AssertReason {
            reason: ProtocolReason::new(
                ProtocolRecordRef::agenda_item(item("item-2")),
                ProtocolRecordRef::contribution(item("item-1"), 0),
                ProtocolReasonKind::FollowsFrom,
                "an empty queue raised the question of scope",
                MemoryConfidence::Medium,
                Some(role("facilitator")),
                at(10),
            )
            .unwrap(),
        }),
        ProtocolCommand::CloseIntervention(CloseIntervention {
            intervention_id: item("item-1"),
            role_id: role("facilitator"),
            now: at(11),
        }),
    ]
}

/// The deferral, the move to `review`, and everything there.
fn review() -> Vec<ProtocolCommand> {
    vec![
        ProtocolCommand::DeferGuard(DeferGuard {
            guard_name: guard("human_approved"),
            content: deferral(),
            deferred_by: role("facilitator"),
            deferred_by_kind: AuditActorKind::Human,
            now: at(12),
        }),
        ProtocolCommand::ApplyTransition(ApplyTransition {
            role_id: Some(role("facilitator")),
            trigger: trigger("submit"),
            now: at(13),
        }),
        ProtocolCommand::StartStep(StartStep {
            role_id: Some(role("facilitator")),
            step_id: step("check"),
            lease: lease("check-1", at(14)),
            now: at(14),
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
            budget_reservation_id: None,
        }),
        ProtocolCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("check"),
            claim_fence: made_core::value_objects::StepClaimFence::new("0".repeat(64)).unwrap(),
            result: StepResult::failed(StepErrorMessage::new("timed out").unwrap()).unwrap(),
            now: at(15),
        }),
        ProtocolCommand::StartStep(StartStep {
            role_id: Some(role("facilitator")),
            step_id: step("check"),
            lease: lease("check-2", at(16)),
            now: at(16),
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
            budget_reservation_id: None,
        }),
        ProtocolCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("check"),
            claim_fence: made_core::value_objects::StepClaimFence::new("0".repeat(64)).unwrap(),
            result: StepResult::completed(readiness(true)).unwrap(),
            now: at(17),
        }),
        ProtocolCommand::ApproveGuard(ApproveGuard {
            guard_name: guard("human_approved"),
            approved_by: role("facilitator"),
            approved_by_kind: AuditActorKind::Human,
            now: at(18),
        }),
        ProtocolCommand::CloseIntervention(CloseIntervention {
            intervention_id: item("item-2"),
            role_id: role("facilitator"),
            now: at(19),
        }),
        ProtocolCommand::ApplyTransition(ApplyTransition {
            role_id: Some(role("facilitator")),
            trigger: trigger("approve"),
            now: at(20),
        }),
    ]
}

#[test]
fn a_session_is_the_fold_of_the_events_its_mutations_decided() {
    let definition = definition();
    let mut stream = ProtocolInstance::decide_start(
        ProtocolId::new("protocol-fold").unwrap(),
        &definition,
        ProtocolContext::empty(),
        None,
        OPENED_AT,
    )
    .expect("required protocol inputs");
    let mut by_events = ProtocolInstance::rehydrate(&stream).unwrap();
    let mut by_mutators = by_events.clone();

    for (position, mut command) in lifecycle().into_iter().enumerate() {
        if let ProtocolCommand::ApplyStepResult(result) = &mut command {
            result.claim_fence = by_events.step_claim_fence(&result.step_id).unwrap();
        }
        let events = by_events
            .decide(&command, &definition)
            .unwrap_or_else(|error| panic!("command {position} refused: {error}"));
        for event in &events {
            by_events.apply(event);
        }
        stream.extend(events);
        mutate(&mut by_mutators, &definition, &command)
            .unwrap_or_else(|error| panic!("mutator {position} refused: {error}"));
        assert_eq!(
            by_events, by_mutators,
            "paths diverge after command {position}"
        );
    }

    assert!(by_mutators.is_completed(&definition));
    assert_eq!(ProtocolInstance::rehydrate(&stream).unwrap(), by_mutators);
    let seen: Vec<AuditEventType> = stream.iter().map(ProtocolEvent::event_type).collect();
    for expected in [
        AuditEventType::ProtocolInstanceStarted,
        AuditEventType::ParticipantsBound,
        AuditEventType::StepStarted,
        AuditEventType::StepCompleted,
        AuditEventType::StepFailed,
        AuditEventType::TransitionApplied,
        AuditEventType::InterventionRequested,
        AuditEventType::InterventionResponded,
        AuditEventType::InterventionClosed,
        AuditEventType::EvidenceCollected,
        AuditEventType::ReasonAsserted,
        AuditEventType::HumanApprovalRecorded,
        AuditEventType::HumanDeferralRecorded,
        AuditEventType::ProtocolCompleted,
    ] {
        assert!(
            seen.contains(&expected),
            "the lifecycle never produced {expected:?}"
        );
    }
}
