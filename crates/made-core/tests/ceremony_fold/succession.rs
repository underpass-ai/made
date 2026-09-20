//! Handing a paused ceremony to a successor.
//!
//! The rules that must hold: a handoff is sealed from a pause and
//! nowhere else; re-sealing the same plan changes nothing and re-sealing
//! a different one conflicts; every outstanding claim is answered for,
//! against the claim the plan saw; evidence is carried only from work
//! this ceremony completed onto a step the successor declares and the
//! diff carries; and a ceremony that sealed a handoff can be cancelled
//! but never resumed.

use made_core::entities::ceremony_commands::{
    ApplyStepResult, CancelCeremony, PauseCeremony, PlanSuccessor, ResumeCeremony, StartStep,
};
use made_core::entities::{
    CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance,
    PublishedCeremonyDefinition,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuditActorId, BudgetDisposition, CarriedEvidence, CeremonyDefinitionDiff, CeremonyId,
    CeremonyVersion, ClaimDisposition, ClaimDispositionKind, DefinitionPin, EventId,
    IdempotencyKey, LifecycleReason, MaxParallel, SourceRecordRef, StepClaimFence, StepId,
    StepOutput, StepResult, SuccessionPlan, SuccessorCeremonyId,
};

use super::fixture::{at, definition, lease, opened, readiness, role, step};

fn apply(instance: &mut CeremonyInstance, events: &[CeremonyEvent]) {
    for event in events {
        instance.apply(event);
    }
}

/// The same ceremony at version two, with `check` still declared.
fn successor_publication() -> PublishedCeremonyDefinition {
    let mut value = serde_json::to_value(definition()).unwrap();
    value["version"] = serde_json::json!("2.0");
    let definition: CeremonyDefinition = serde_json::from_value(value).unwrap();
    PublishedCeremonyDefinition::seal(definition).unwrap()
}

/// Version two with `check` removed: a session that has not run it
/// never will, which is what the diff calls stranding.
fn stranding_publication() -> PublishedCeremonyDefinition {
    let mut value = serde_json::to_value(definition()).unwrap();
    value["version"] = serde_json::json!("2.0");
    value["steps"]["check"]["handler_kind"] = serde_json::json!("other_handler");
    value["steps"]["check"]["state_id"] = serde_json::json!("done");
    let definition: CeremonyDefinition = serde_json::from_value(value).unwrap();
    PublishedCeremonyDefinition::seal(definition).unwrap()
}

fn pin(published: &PublishedCeremonyDefinition) -> DefinitionPin {
    DefinitionPin::new(
        published.name().clone(),
        published.version().clone(),
        published.digest(),
    )
}

fn diff_against(published: &PublishedCeremonyDefinition) -> CeremonyDefinitionDiff {
    CeremonyDefinitionDiff::between(&definition(), published.definition())
}

fn plan_id() -> IdempotencyKey {
    IdempotencyKey::new("handoff-1").unwrap()
}

fn successor_id(instance: &CeremonyInstance) -> CeremonyId {
    SuccessorCeremonyId::derive(instance.id(), &plan_id())
        .unwrap()
        .into_ceremony_id()
}

fn plan(
    instance: &CeremonyInstance,
    published: &PublishedCeremonyDefinition,
    carried: Vec<CarriedEvidence>,
    dispositions: Vec<ClaimDisposition>,
) -> SuccessionPlan {
    SuccessionPlan::new(
        plan_id(),
        successor_id(instance),
        pin(published),
        carried,
        dispositions,
        BudgetDisposition::Fresh,
        AuditActorId::new("operator-1"),
        at(30),
    )
}

fn command(plan: SuccessionPlan, published: &PublishedCeremonyDefinition) -> CeremonyCommand {
    CeremonyCommand::PlanSuccessor(PlanSuccessor {
        plan,
        successor: Box::new(published.clone()),
        diff: diff_against(published),
        now: at(30),
    })
}

/// A paused ceremony whose `plan` step completed and whose `check`
/// step is still claimed.
fn paused_with_one_live_claim() -> (CeremonyInstance, CeremonyDefinition, StepClaimFence) {
    let definition = definition();
    let mut instance = opened(&definition);
    let started = instance
        .decide(
            &CeremonyCommand::StartStep(StartStep {
                role_id: Some(role("facilitator")),
                step_id: step("plan"),
                lease: lease("plan-claim", at(1)),
                now: at(1),
                max_parallel_ceiling: MaxParallel::SERVER_MAX,
                budget_reservation_id: None,
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &started);
    let fence = instance.step_claim_fence(&step("plan")).unwrap();
    let completed = instance
        .decide(
            &CeremonyCommand::ApplyStepResult(ApplyStepResult {
                step_id: step("plan"),
                claim_fence: fence,
                result: StepResult::completed(readiness(true)).unwrap(),
                now: at(2),
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &completed);

    let moved = instance
        .decide(
            &CeremonyCommand::ApplyTransition(
                made_core::entities::ceremony_commands::ApplyTransition {
                    role_id: Some(role("facilitator")),
                    trigger: super::fixture::trigger("submit"),
                    now: at(3),
                },
            ),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &moved);

    let claimed = instance
        .decide(
            &CeremonyCommand::StartStep(StartStep {
                role_id: Some(role("facilitator")),
                step_id: step("check"),
                lease: lease("check-claim", at(4)),
                now: at(4),
                max_parallel_ceiling: MaxParallel::SERVER_MAX,
                budget_reservation_id: None,
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &claimed);
    let check_fence = instance.step_claim_fence(&step("check")).unwrap();

    let paused = instance
        .decide(
            &CeremonyCommand::PauseCeremony(PauseCeremony {
                reason: LifecycleReason::new("the definition is wrong").unwrap(),
                now: at(5),
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &paused);
    (instance, definition, check_fence)
}

fn retry(step_id: StepId, fence: StepClaimFence) -> ClaimDisposition {
    ClaimDisposition::new(step_id, fence, ClaimDispositionKind::RetryInSuccessor)
}

fn carried_plan_step(instance: &CeremonyInstance) -> CarriedEvidence {
    CarriedEvidence::new(
        step("plan"),
        SourceRecordRef::new(
            instance.id().clone(),
            step("plan"),
            EventId::new("ceremony-fold:step_completed:plan").unwrap(),
            made_core::value_objects::AuditRecordHash::from_bytes([7; 32]),
            instance.step_record(&step("plan")).unwrap().state_visit(),
            instance.step_record(&step("plan")).unwrap().attempt(),
        ),
        readiness(true),
    )
}

#[test]
fn a_handoff_is_sealed_from_a_pause_and_nowhere_else() {
    let definition = definition();
    let running = opened(&definition);
    let published = successor_publication();
    let refused = running
        .decide(
            &command(
                plan(&running, &published, Vec::new(), Vec::new()),
                &published,
            ),
            &definition,
        )
        .unwrap_err();

    assert!(matches!(
        refused,
        DomainError::LifecycleRefused {
            operation: "plan_successor",
            ..
        }
    ));

    let (paused, definition, fence) = paused_with_one_live_claim();
    let sealed = paused
        .decide(
            &command(
                plan(
                    &paused,
                    &published,
                    vec![carried_plan_step(&paused)],
                    vec![retry(step("check"), fence)],
                ),
                &published,
            ),
            &definition,
        )
        .unwrap();

    assert!(matches!(
        sealed.as_slice(),
        [CeremonyEvent::SuccessorPlanned(_)]
    ));
    let mut folded = paused.clone();
    apply(&mut folded, &sealed);
    assert!(folded.is_superseded());
    assert_eq!(
        folded.successor_plan().unwrap().successor_id(),
        &successor_id(&paused)
    );
    let _ = running.id();
}

#[test]
fn re_sealing_the_same_plan_changes_nothing_and_a_different_one_conflicts() {
    let (paused, definition, fence) = paused_with_one_live_claim();
    let published = successor_publication();
    let sealed = plan(
        &paused,
        &published,
        vec![carried_plan_step(&paused)],
        vec![retry(step("check"), fence.clone())],
    );
    let mut folded = paused.clone();
    let events = folded
        .decide(&command(sealed.clone(), &published), &definition)
        .unwrap();
    apply(&mut folded, &events);

    assert!(folded
        .decide(&command(sealed, &published), &definition)
        .unwrap()
        .is_empty());

    let other = plan(
        &paused,
        &published,
        Vec::new(),
        vec![ClaimDisposition::new(
            step("check"),
            fence,
            ClaimDispositionKind::AbandonNoExternalEffect,
        )],
    );
    assert!(matches!(
        folded
            .decide(&command(other, &published), &definition)
            .unwrap_err(),
        DomainError::Conflict {
            what: "successor_already_planned"
        }
    ));
}

#[test]
fn an_outstanding_claim_without_a_disposition_fails_the_plan_by_name() {
    let (paused, definition, _) = paused_with_one_live_claim();
    let published = successor_publication();
    let refused = paused
        .decide(
            &command(
                plan(&paused, &published, Vec::new(), Vec::new()),
                &published,
            ),
            &definition,
        )
        .unwrap_err();

    let DomainError::InvalidDocument { reason } = refused else {
        panic!("an undisposed claim is a document defect: {refused:?}");
    };
    assert!(reason.contains("check"), "{reason}");
}

#[test]
fn a_disposition_for_a_claim_that_moved_on_is_refused() {
    let (paused, definition, _) = paused_with_one_live_claim();
    let published = successor_publication();
    let stale = StepClaimFence::new("a".repeat(64)).unwrap();
    let refused = paused
        .decide(
            &command(
                plan(
                    &paused,
                    &published,
                    Vec::new(),
                    vec![retry(step("check"), stale)],
                ),
                &published,
            ),
            &definition,
        )
        .unwrap_err();

    assert!(matches!(refused, DomainError::InvariantViolated { .. }));
}

#[test]
fn evidence_cannot_be_carried_onto_a_step_the_successor_strands() {
    let (paused, definition, fence) = paused_with_one_live_claim();
    let published = stranding_publication();
    let carried = CarriedEvidence::new(
        step("check"),
        SourceRecordRef::new(
            paused.id().clone(),
            step("plan"),
            EventId::new("ceremony-fold:step_completed:plan").unwrap(),
            made_core::value_objects::AuditRecordHash::from_bytes([7; 32]),
            paused.step_record(&step("plan")).unwrap().state_visit(),
            paused.step_record(&step("plan")).unwrap().attempt(),
        ),
        readiness(true),
    );
    let refused = paused
        .decide(
            &command(
                plan(
                    &paused,
                    &published,
                    vec![carried],
                    vec![retry(step("check"), fence)],
                ),
                &published,
            ),
            &definition,
        )
        .unwrap_err();

    let DomainError::InvalidDocument { reason } = refused else {
        panic!("a stranded carry is a document defect: {refused:?}");
    };
    assert!(reason.contains("stranded"), "{reason}");
}

#[test]
fn evidence_cannot_be_carried_from_work_this_ceremony_did_not_complete() {
    let (paused, definition, fence) = paused_with_one_live_claim();
    let published = successor_publication();
    let carried = CarriedEvidence::new(
        step("check"),
        SourceRecordRef::new(
            paused.id().clone(),
            step("check"),
            EventId::new("ceremony-fold:step_completed:check").unwrap(),
            made_core::value_objects::AuditRecordHash::from_bytes([7; 32]),
            paused.step_record(&step("check")).unwrap().state_visit(),
            paused.step_record(&step("check")).unwrap().attempt(),
        ),
        StepOutput::empty(),
    );
    let refused = paused
        .decide(
            &command(
                plan(
                    &paused,
                    &published,
                    vec![carried],
                    vec![retry(step("check"), fence)],
                ),
                &published,
            ),
            &definition,
        )
        .unwrap_err();

    let DomainError::InvalidDocument { reason } = refused else {
        panic!("carrying unfinished work is a document defect: {refused:?}");
    };
    assert!(reason.contains("did not complete"), "{reason}");
}

#[test]
fn a_transferred_budget_is_refused_with_its_reason() {
    let (paused, definition, fence) = paused_with_one_live_claim();
    let published = successor_publication();
    let transferring = SuccessionPlan::new(
        plan_id(),
        successor_id(&paused),
        pin(&published),
        Vec::new(),
        vec![retry(step("check"), fence)],
        BudgetDisposition::TransferRemaining,
        AuditActorId::new("operator-1"),
        at(30),
    );

    assert!(matches!(
        paused
            .decide(&command(transferring, &published), &definition)
            .unwrap_err(),
        DomainError::InvariantViolated {
            reason: "budget transfer is not supported in this release"
        }
    ));
}

#[test]
fn a_superseded_ceremony_cannot_resume_but_can_still_be_cancelled() {
    let (paused, definition, fence) = paused_with_one_live_claim();
    let published = successor_publication();
    let mut folded = paused.clone();
    let events = folded
        .decide(
            &command(
                plan(
                    &paused,
                    &published,
                    vec![carried_plan_step(&paused)],
                    vec![retry(step("check"), fence)],
                ),
                &published,
            ),
            &definition,
        )
        .unwrap();
    apply(&mut folded, &events);

    let refused = folded
        .decide(
            &CeremonyCommand::ResumeCeremony(ResumeCeremony { now: at(40) }),
            &definition,
        )
        .unwrap_err();
    assert!(matches!(
        refused,
        DomainError::LifecycleRefused {
            operation: "superseded_by_successor",
            ..
        }
    ));

    let cancelled = folded
        .decide(
            &CeremonyCommand::CancelCeremony(CancelCeremony {
                reason: LifecycleReason::new("superseded").unwrap(),
                now: at(41),
            }),
            &definition,
        )
        .unwrap();
    assert!(matches!(
        cancelled.as_slice(),
        [CeremonyEvent::CeremonyCancelled(_)]
    ));
}

#[test]
fn a_successor_opens_holding_what_it_was_given_and_nothing_it_did_itself() {
    let (paused, _, fence) = paused_with_one_live_claim();
    let published = successor_publication();
    let sealed = plan(
        &paused,
        &published,
        vec![carried_plan_step(&paused)],
        vec![retry(step("check"), fence)],
    );
    let succession = made_core::value_objects::CeremonySuccession::new(
        paused.id().clone(),
        made_core::value_objects::AuditRecordHash::from_bytes([9; 32]),
        made_core::value_objects::StreamVersion::new(7),
        DefinitionPin::new(
            paused.definition_name().clone(),
            CeremonyVersion::v1(),
            definition().digest().unwrap(),
        ),
        pin(&published),
        plan_id(),
    );
    let opening = CeremonyInstance::decide_start_successor(
        successor_id(&paused),
        &published,
        paused.context().clone(),
        succession.clone(),
        &sealed,
        at(31),
    )
    .unwrap();

    assert!(matches!(
        opening.as_slice(),
        [
            CeremonyEvent::CeremonyInstanceStarted(_),
            CeremonyEvent::SuccessionCarried(_)
        ]
    ));
    let successor = CeremonyInstance::rehydrate(&opening).unwrap();
    assert_eq!(successor.succession(), Some(&succession));
    let carried = successor.step_record(&step("plan")).unwrap();
    assert_eq!(carried.output(), &readiness(true));
    assert_eq!(
        carried.carried_from().map(SourceRecordRef::ceremony_id),
        Some(paused.id())
    );
    // The successor never ran it, so it never claims to have retried it.
    assert_eq!(carried.attempt().get(), 1);
    assert!(successor
        .step_record(&step("check"))
        .unwrap()
        .carried_from()
        .is_none());
    // Deciding the opening again derives the same batch, which is what
    // lets a retry verify the stream instead of opening a second one.
    assert_eq!(
        CeremonyInstance::decide_start_successor(
            successor_id(&paused),
            &published,
            paused.context().clone(),
            succession,
            &sealed,
            at(31),
        )
        .unwrap(),
        opening
    );
}
