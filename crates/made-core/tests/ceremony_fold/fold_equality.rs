//! After a whole lifecycle of mutations, the session is the fold of
//! the events those mutations decided — and the mutators and the
//! decide-then-apply path agree at every step.

use made_core::entities::ceremony_commands::{
    ApplyStepResult, ApplyTransition, ApproveGuard, AssertReason, BindParticipant,
    CloseIntervention, DeferGuard, RequestIntervention, RespondToIntervention,
    RespondToInterventionWithEvidence, StartStep,
};
use made_core::entities::{CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuditActorKind, AuditEventType, CeremonyContext, CeremonyId, CeremonyInterventionKind,
    CeremonyInterventionTarget, CeremonyReason, CeremonyReasonKind, CeremonyRecordRef,
    MemoryConfidence, StepErrorMessage, StepResult,
};

use super::fixture::{
    at, content, deferral, definition, evidence_pack, guard, item, lease, readiness, role,
    specialty, step, trigger, OPENED_AT,
};

/// Run the legacy mutator that `command` replaces.
pub(crate) fn mutate(
    instance: &mut CeremonyInstance,
    definition: &CeremonyDefinition,
    command: &CeremonyCommand,
) -> Result<(), DomainError> {
    match command {
        CeremonyCommand::BindParticipant(c) => {
            instance.bind_participant(definition, c.role_id.clone(), c.specialty.clone(), c.now)
        }
        CeremonyCommand::StartStep(c) => start_step(instance, definition, c),
        CeremonyCommand::ApplyStepResult(c) => {
            instance.apply_step_result(definition, &c.step_id, c.result.clone(), c.now)
        }
        CeremonyCommand::ApplyTransition(c) => apply_transition(instance, definition, c),
        CeremonyCommand::ApproveGuard(c) => instance.approve_guard(
            definition,
            &c.guard_name,
            c.approved_by.clone(),
            c.approved_by_kind,
            c.now,
        ),
        CeremonyCommand::DeferGuard(c) => instance.defer_guard(
            definition,
            c.guard_name.clone(),
            c.content.clone(),
            c.deferred_by.clone(),
            c.deferred_by_kind,
            c.now,
        ),
        CeremonyCommand::RequestIntervention(c) => request_intervention(instance, definition, c),
        CeremonyCommand::RespondToIntervention(c) => instance.respond_to_intervention_as(
            definition,
            &c.intervention_id,
            c.role_id.clone(),
            c.content.clone(),
            c.now,
        ),
        CeremonyCommand::RespondToInterventionWithEvidence(c) => instance
            .respond_to_intervention_with_evidence_as(
                definition,
                &c.intervention_id,
                c.role_id.clone(),
                c.evidence_pack.clone(),
                c.now,
            ),
        CeremonyCommand::AssertReason(c) => assert_reason(instance, definition, &c.reason),
        CeremonyCommand::CloseIntervention(c) => {
            instance.close_intervention_as(definition, &c.intervention_id, &c.role_id, c.now)
        }
    }
}

fn start_step(
    instance: &mut CeremonyInstance,
    definition: &CeremonyDefinition,
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
    instance: &mut CeremonyInstance,
    definition: &CeremonyDefinition,
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
    instance: &mut CeremonyInstance,
    definition: &CeremonyDefinition,
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
    instance: &mut CeremonyInstance,
    definition: &CeremonyDefinition,
    reason: &CeremonyReason,
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
fn lifecycle() -> Vec<CeremonyCommand> {
    let mut commands = drafting();
    commands.extend(review());
    commands
}

/// Everything that happens in `drafting`.
fn drafting() -> Vec<CeremonyCommand> {
    let observer = CeremonyInterventionTarget::roles([role("observer")]).unwrap();
    vec![
        CeremonyCommand::BindParticipant(BindParticipant {
            role_id: role("observer"),
            specialty: specialty("queues"),
            now: at(1),
        }),
        CeremonyCommand::StartStep(StartStep {
            role_id: Some(role("facilitator")),
            step_id: step("plan"),
            lease: lease("plan-1", at(2)),
            now: at(2),
        }),
        CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            result: StepResult::completed(readiness(false)).unwrap(),
            now: at(3),
        }),
        CeremonyCommand::StartStep(StartStep {
            role_id: None,
            step_id: step("plan"),
            lease: lease("plan-2", at(4)),
            now: at(4),
        }),
        CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("plan"),
            result: StepResult::completed(readiness(true)).unwrap(),
            now: at(5),
        }),
        CeremonyCommand::RequestIntervention(RequestIntervention {
            intervention_id: item("item-1"),
            role_id: role("facilitator"),
            kind: CeremonyInterventionKind::Investigation,
            target: observer.clone(),
            content: content("Look at the queue."),
            provenance: None,
            now: at(6),
        }),
        CeremonyCommand::RespondToIntervention(RespondToIntervention {
            intervention_id: item("item-1"),
            role_id: role("observer"),
            content: content("It is empty."),
            now: at(7),
        }),
        CeremonyCommand::RequestIntervention(RequestIntervention {
            intervention_id: item("item-2"),
            role_id: role("facilitator"),
            kind: CeremonyInterventionKind::Investigation,
            target: observer,
            content: content("What does the wiki say?"),
            provenance: None,
            now: at(8),
        }),
        CeremonyCommand::RespondToInterventionWithEvidence(RespondToInterventionWithEvidence {
            intervention_id: item("item-2"),
            role_id: role("observer"),
            evidence_pack: evidence_pack("wiki"),
            now: at(9),
        }),
        CeremonyCommand::AssertReason(AssertReason {
            reason: CeremonyReason::new(
                CeremonyRecordRef::agenda_item(item("item-2")),
                CeremonyRecordRef::contribution(item("item-1"), 0),
                CeremonyReasonKind::FollowsFrom,
                "an empty queue raised the question of scope",
                MemoryConfidence::Medium,
                Some(role("facilitator")),
                at(10),
            )
            .unwrap(),
        }),
        CeremonyCommand::CloseIntervention(CloseIntervention {
            intervention_id: item("item-1"),
            role_id: role("facilitator"),
            now: at(11),
        }),
    ]
}

/// The deferral, the move to `review`, and everything there.
fn review() -> Vec<CeremonyCommand> {
    vec![
        CeremonyCommand::DeferGuard(DeferGuard {
            guard_name: guard("human_approved"),
            content: deferral(),
            deferred_by: role("facilitator"),
            deferred_by_kind: AuditActorKind::Human,
            now: at(12),
        }),
        CeremonyCommand::ApplyTransition(ApplyTransition {
            role_id: Some(role("facilitator")),
            trigger: trigger("submit"),
            now: at(13),
        }),
        CeremonyCommand::StartStep(StartStep {
            role_id: Some(role("facilitator")),
            step_id: step("check"),
            lease: lease("check-1", at(14)),
            now: at(14),
        }),
        CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("check"),
            result: StepResult::failed(StepErrorMessage::new("timed out").unwrap()).unwrap(),
            now: at(15),
        }),
        CeremonyCommand::StartStep(StartStep {
            role_id: Some(role("facilitator")),
            step_id: step("check"),
            lease: lease("check-2", at(16)),
            now: at(16),
        }),
        CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("check"),
            result: StepResult::completed(readiness(true)).unwrap(),
            now: at(17),
        }),
        CeremonyCommand::ApproveGuard(ApproveGuard {
            guard_name: guard("human_approved"),
            approved_by: role("facilitator"),
            approved_by_kind: AuditActorKind::Human,
            now: at(18),
        }),
        CeremonyCommand::CloseIntervention(CloseIntervention {
            intervention_id: item("item-2"),
            role_id: role("facilitator"),
            now: at(19),
        }),
        CeremonyCommand::ApplyTransition(ApplyTransition {
            role_id: Some(role("facilitator")),
            trigger: trigger("approve"),
            now: at(20),
        }),
    ]
}

#[test]
fn a_session_is_the_fold_of_the_events_its_mutations_decided() {
    let definition = definition();
    let mut stream = CeremonyInstance::decide_start(
        CeremonyId::new("ceremony-fold").unwrap(),
        &definition,
        CeremonyContext::empty(),
        None,
        OPENED_AT,
    )
    .expect("required ceremony inputs");
    let mut by_events = CeremonyInstance::rehydrate(&stream).unwrap();
    let mut by_mutators = by_events.clone();

    for (position, command) in lifecycle().iter().enumerate() {
        let events = by_events
            .decide(command, &definition)
            .unwrap_or_else(|error| panic!("command {position} refused: {error}"));
        for event in &events {
            by_events.apply(event);
        }
        stream.extend(events);
        mutate(&mut by_mutators, &definition, command)
            .unwrap_or_else(|error| panic!("mutator {position} refused: {error}"));
        assert_eq!(
            by_events, by_mutators,
            "paths diverge after command {position}"
        );
    }

    assert!(by_mutators.is_completed(&definition));
    assert_eq!(CeremonyInstance::rehydrate(&stream).unwrap(), by_mutators);
    let seen: Vec<AuditEventType> = stream.iter().map(CeremonyEvent::event_type).collect();
    for expected in [
        AuditEventType::CeremonyInstanceStarted,
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
        AuditEventType::CeremonyCompleted,
    ] {
        assert!(
            seen.contains(&expected),
            "the lifecycle never produced {expected:?}"
        );
    }
}
