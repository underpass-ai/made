//! `CeremonyInstance::decide` yields, field for field, the events the
//! `session_events` builders read off a mutated session.
//!
//! The builders are how the journal learns what happened today; the
//! use cases will replace them with `decide`. For every builder, the
//! legacy path — mutate, then build from the session and the use
//! case's arguments — and the new path — decide on the session before
//! the mutation — must produce equal events. This is the safety net
//! for that replacement.

use std::collections::BTreeMap;

use made_core::entities::ceremony_commands::{
    ApplyStepResult, ApplyTransition, ApproveGuard, AssertReason, BindParticipant,
    CloseIntervention, DeferGuard, RequestIntervention, RespondToIntervention,
    RespondToInterventionWithEvidence, StartStep,
};
use made_core::entities::{
    CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyEvidencePack, CeremonyInstance,
    ContextItem, ContextSummary, ExternalContextBundle, PublishedCeremonyDefinition,
};
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyContext, CeremonyEvidenceSourceId,
    CeremonyGuardDeferralContent, CeremonyInterventionContent, CeremonyInterventionId,
    CeremonyInterventionKind, CeremonyInterventionTarget, CeremonyReason, CeremonyReasonKind,
    CeremonyRecordRef, GuardName, MemoryConfidence, Specialty, StepErrorMessage, StepLease,
    StepOutput, StepResult,
};
use serde_json::json;

use super::ceremony_test_support::{
    approval_definition, ceremony_id, definition, idempotency_key, lease_owner, lease_ttl, now,
    repeating_definition, respondent_role_id, role_id, started_instance, step_id, trigger,
    two_step_definition,
};
use crate::services::session_events;

fn lease(key: &str) -> StepLease {
    StepLease::acquire(lease_owner(), idempotency_key(key), now(), lease_ttl()).unwrap()
}

fn content(message: &str) -> CeremonyInterventionContent {
    CeremonyInterventionContent::new(message, Attributes::empty()).unwrap()
}

fn intervention_id() -> CeremonyInterventionId {
    CeremonyInterventionId::new("item-1").unwrap()
}

fn human_approved() -> GuardName {
    GuardName::new("human_approved").unwrap()
}

fn readiness(ready: bool) -> StepOutput {
    StepOutput::new(Attributes::new(BTreeMap::from([("ready".to_owned(), json!(ready))])).unwrap())
}

fn evidence_pack() -> CeremonyEvidencePack {
    let bundle = ExternalContextBundle::new(
        "bundle-1",
        "1.0",
        Some(ContextSummary::new("The wiki fixes the scope.", Attributes::empty()).unwrap()),
        vec![ContextItem::new(
            "scope",
            "page",
            "Scope",
            Some("The scope is fixed.".to_owned()),
            Attributes::empty(),
            Vec::new(),
        )
        .unwrap()],
        Vec::new(),
        Attributes::empty(),
    )
    .unwrap();
    CeremonyEvidencePack::new(
        CeremonyEvidenceSourceId::new("wiki").unwrap(),
        bundle,
        now(),
    )
    .unwrap()
}

fn with_step_in_progress(definition: &CeremonyDefinition) -> CeremonyInstance {
    let mut instance = started_instance(definition);
    instance
        .start_step_as(definition, &role_id(), &step_id(), lease("lease-1"), now())
        .unwrap();
    instance
}

fn with_open_item(definition: &CeremonyDefinition) -> CeremonyInstance {
    let mut instance = started_instance(definition);
    instance
        .request_intervention_as(
            definition,
            intervention_id(),
            role_id(),
            CeremonyInterventionKind::Investigation,
            CeremonyInterventionTarget::roles([respondent_role_id()]).unwrap(),
            content("Look at the queue."),
            now(),
        )
        .unwrap();
    instance
}

fn with_answered_item(definition: &CeremonyDefinition) -> CeremonyInstance {
    let mut instance = with_open_item(definition);
    instance
        .respond_to_intervention_as(
            definition,
            &intervention_id(),
            respondent_role_id(),
            content("It is empty."),
            now(),
        )
        .unwrap();
    instance
}

#[test]
fn ceremony_started() {
    let definition = definition();

    let instance = started_instance(&definition);
    let built = session_events::ceremony_started(&instance);
    let decided =
        CeremonyInstance::decide_start(ceremony_id(), &definition, CeremonyContext::empty(), now());
    assert_eq!(decided, built);

    let published = PublishedCeremonyDefinition::seal(definition).unwrap();
    let bound =
        CeremonyInstance::start_bound(ceremony_id(), &published, CeremonyContext::empty(), now());
    let built = session_events::ceremony_started(&bound);
    let decided = CeremonyInstance::decide_start_bound(
        ceremony_id(),
        &published,
        CeremonyContext::empty(),
        now(),
    );
    assert_eq!(decided, built);
}

#[test]
fn participants_bound() {
    let definition = definition();
    let before = started_instance(&definition);
    let specialty = Specialty::new("queues").unwrap();

    let decided = before
        .decide(
            &CeremonyCommand::BindParticipant(BindParticipant {
                role_id: role_id(),
                specialty: specialty.clone(),
                now: now(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .bind_participant(&definition, role_id(), specialty.clone(), now())
        .unwrap();
    let seating = BTreeMap::from([(role_id(), specialty)]);
    let built = session_events::participants_bound(&after, &seating).unwrap();
    assert_eq!(decided, vec![built]);
}

#[test]
fn step_started() {
    let definition = definition();
    let before = started_instance(&definition);

    let decided = before
        .decide(
            &CeremonyCommand::StartStep(StartStep {
                role_id: Some(role_id()),
                step_id: step_id(),
                lease: lease("lease-1"),
                now: now(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    let attempt = after
        .start_step_as(&definition, &role_id(), &step_id(), lease("lease-1"), now())
        .unwrap();
    let iteration = after.step_record(&step_id()).unwrap().iteration();
    let built =
        session_events::step_started(&after, &step_id(), iteration, attempt, &role_id(), now())
            .unwrap();
    assert_eq!(decided, vec![built]);
}

/// The legacy path: the coordinates captured before the result is
/// applied, the seat the definition assigns to the step, the event
/// read off the mutated session.
fn step_finished_by_the_builder(
    definition: &CeremonyDefinition,
    before: &CeremonyInstance,
    result: &StepResult,
) -> CeremonyEvent {
    let record = before.step_record(&step_id()).unwrap();
    let (iteration, attempt) = (record.iteration(), record.attempt());
    let finished_by = definition.role_id_for_step(&step_id()).unwrap();
    let mut after = before.clone();
    after
        .apply_step_result(definition, &step_id(), result.clone(), now())
        .unwrap();
    session_events::step_finished(
        &after,
        &step_id(),
        iteration,
        attempt,
        result,
        &finished_by,
        now(),
    )
}

fn step_finished_by_decide(
    definition: &CeremonyDefinition,
    before: &CeremonyInstance,
    result: &StepResult,
) -> Vec<CeremonyEvent> {
    before
        .decide(
            &CeremonyCommand::ApplyStepResult(ApplyStepResult {
                step_id: step_id(),
                result: result.clone(),
                now: now(),
            }),
            definition,
        )
        .unwrap()
}

#[test]
fn step_finished() {
    let definition = definition();
    let before = with_step_in_progress(&definition);
    for result in [
        StepResult::completed(StepOutput::empty()).unwrap(),
        StepResult::failed(StepErrorMessage::new("no voices").unwrap()).unwrap(),
    ] {
        assert_eq!(
            step_finished_by_decide(&definition, &before, &result),
            vec![step_finished_by_the_builder(&definition, &before, &result)]
        );
    }

    // A success that reopens the step carries the next iteration.
    let repeating = repeating_definition(2);
    let before = with_step_in_progress(&repeating);
    let result = StepResult::completed(readiness(false)).unwrap();
    let decided = step_finished_by_decide(&repeating, &before, &result);
    assert_eq!(
        decided,
        vec![step_finished_by_the_builder(&repeating, &before, &result)]
    );
    let [CeremonyEvent::StepCompleted(completed)] = decided.as_slice() else {
        panic!("expected one completion, got {decided:?}");
    };
    assert!(completed.next_iteration.is_some());
}

#[test]
fn transition_applied_and_ceremony_completed() {
    let definition = definition();
    let mut before = with_step_in_progress(&definition);
    before
        .apply_step_result(
            &definition,
            &step_id(),
            StepResult::completed(StepOutput::empty()).unwrap(),
            now(),
        )
        .unwrap();

    let decided = before
        .decide(
            &CeremonyCommand::ApplyTransition(ApplyTransition {
                role_id: Some(role_id()),
                trigger: trigger(),
                now: now(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .apply_transition_as(&definition, &role_id(), &trigger(), now())
        .unwrap();
    assert!(after.is_terminal(&definition));
    let built = vec![
        session_events::transition_applied(&after).unwrap(),
        session_events::ceremony_completed(&after).unwrap(),
    ];
    assert_eq!(decided, built);
}

#[test]
fn transition_applied_short_of_the_end() {
    let definition = two_step_definition();
    let open = made_core::value_objects::StepId::new("open").unwrap();
    let opened = made_core::value_objects::TransitionTrigger::new("opened").unwrap();
    let mut before = started_instance(&definition);
    before
        .start_step_as(&definition, &role_id(), &open, lease("lease-1"), now())
        .unwrap();
    before
        .apply_step_result(
            &definition,
            &open,
            StepResult::completed(StepOutput::empty()).unwrap(),
            now(),
        )
        .unwrap();

    let decided = before
        .decide(
            &CeremonyCommand::ApplyTransition(ApplyTransition {
                role_id: Some(role_id()),
                trigger: opened.clone(),
                now: now(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .apply_transition_as(&definition, &role_id(), &opened, now())
        .unwrap();
    assert!(!after.is_terminal(&definition));
    let built = vec![session_events::transition_applied(&after).unwrap()];
    assert_eq!(decided, built);
}

#[test]
fn intervention_requested() {
    let definition = definition();
    let before = started_instance(&definition);
    let target = CeremonyInterventionTarget::roles([respondent_role_id()]).unwrap();

    let decided = before
        .decide(
            &CeremonyCommand::RequestIntervention(RequestIntervention {
                intervention_id: intervention_id(),
                role_id: role_id(),
                kind: CeremonyInterventionKind::Investigation,
                target: target.clone(),
                content: content("Look at the queue."),
                provenance: None,
                now: now(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .request_intervention_as(
            &definition,
            intervention_id(),
            role_id(),
            CeremonyInterventionKind::Investigation,
            target,
            content("Look at the queue."),
            now(),
        )
        .unwrap();
    let built = session_events::intervention_requested(&after, &intervention_id()).unwrap();
    assert_eq!(decided, vec![built]);
}

#[test]
fn intervention_responded() {
    let definition = definition();
    let before = with_open_item(&definition);

    let decided = before
        .decide(
            &CeremonyCommand::RespondToIntervention(RespondToIntervention {
                intervention_id: intervention_id(),
                role_id: respondent_role_id(),
                content: content("It is empty."),
                now: now(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .respond_to_intervention_as(
            &definition,
            &intervention_id(),
            respondent_role_id(),
            content("It is empty."),
            now(),
        )
        .unwrap();
    let built =
        session_events::intervention_responded(&after, &intervention_id(), &respondent_role_id())
            .unwrap();
    assert_eq!(decided, vec![built]);
}

#[test]
fn intervention_closed() {
    let definition = definition();
    let before = with_answered_item(&definition);

    let decided = before
        .decide(
            &CeremonyCommand::CloseIntervention(CloseIntervention {
                intervention_id: intervention_id(),
                role_id: role_id(),
                now: now(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .close_intervention_as(&definition, &intervention_id(), &role_id(), now())
        .unwrap();
    let built =
        session_events::intervention_closed(&after, &intervention_id(), &role_id()).unwrap();
    assert_eq!(decided, vec![built]);
}

/// Two facts, in the order the journal seals them: the source was
/// consulted, then the item was answered.
#[test]
fn evidence_collected() {
    let definition = definition();
    let before = with_open_item(&definition);
    let pack = evidence_pack();

    let decided = before
        .decide(
            &CeremonyCommand::RespondToInterventionWithEvidence(
                RespondToInterventionWithEvidence {
                    intervention_id: intervention_id(),
                    role_id: respondent_role_id(),
                    evidence_pack: pack.clone(),
                    now: now(),
                },
            ),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .respond_to_intervention_with_evidence_as(
            &definition,
            &intervention_id(),
            respondent_role_id(),
            pack.clone(),
            now(),
        )
        .unwrap();
    let built = vec![
        session_events::evidence_collected(
            &after,
            &intervention_id(),
            pack.source_id(),
            &respondent_role_id(),
            now(),
        )
        .unwrap(),
        session_events::intervention_responded(&after, &intervention_id(), &respondent_role_id())
            .unwrap(),
    ];
    assert_eq!(decided, built);
}

#[test]
fn reason_asserted() {
    let definition = definition();
    let before = with_answered_item(&definition);
    let from = CeremonyRecordRef::contribution(intervention_id(), 0);
    let to = CeremonyRecordRef::agenda_item(intervention_id());

    let decided = before
        .decide(
            &CeremonyCommand::AssertReason(AssertReason {
                reason: CeremonyReason::new(
                    from.clone(),
                    to.clone(),
                    CeremonyReasonKind::ChosenBecause,
                    "the queue was visibly empty",
                    MemoryConfidence::High,
                    Some(respondent_role_id()),
                    now(),
                )
                .unwrap(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .assert_reason_as(
            &definition,
            respondent_role_id(),
            from,
            to,
            CeremonyReasonKind::ChosenBecause,
            "the queue was visibly empty",
            MemoryConfidence::High,
            now(),
        )
        .unwrap();
    let built = session_events::reason_asserted(&after).unwrap();
    assert_eq!(decided, vec![built]);
}

#[test]
fn guard_approved() {
    let definition = approval_definition();
    let before = started_instance(&definition);

    let decided = before
        .decide(
            &CeremonyCommand::ApproveGuard(ApproveGuard {
                guard_name: human_approved(),
                approved_by: role_id(),
                approved_by_kind: AuditActorKind::Human,
                now: now(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .approve_guard(
            &definition,
            &human_approved(),
            role_id(),
            AuditActorKind::Human,
            now(),
        )
        .unwrap();
    let built = session_events::guard_approved(&after, &human_approved()).unwrap();
    assert_eq!(decided, vec![built]);
}

#[test]
fn guard_deferred() {
    let definition = approval_definition();
    let before = started_instance(&definition);
    let deferral = CeremonyGuardDeferralContent::new(
        "Not yet.",
        "The plan has not been read.",
        vec!["The plan is read.".to_owned()],
    )
    .unwrap();

    let decided = before
        .decide(
            &CeremonyCommand::DeferGuard(DeferGuard {
                guard_name: human_approved(),
                content: deferral.clone(),
                deferred_by: role_id(),
                deferred_by_kind: AuditActorKind::Human,
                now: now(),
            }),
            &definition,
        )
        .unwrap();

    let mut after = before;
    after
        .defer_guard(
            &definition,
            human_approved(),
            deferral,
            role_id(),
            AuditActorKind::Human,
            now(),
        )
        .unwrap();
    let built = session_events::guard_deferred(&after, &human_approved()).unwrap();
    assert_eq!(decided, vec![built]);
}
