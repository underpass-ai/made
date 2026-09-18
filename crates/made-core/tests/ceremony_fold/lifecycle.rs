use made_core::entities::ceremony_commands::{
    ApplyStepResult, CancelCeremony, EnforceCeremonyDeadlines, PauseCeremony, ResumeCeremony,
    StartStep,
};
use made_core::entities::ceremony_events::CeremonyCompleted;
use made_core::entities::{CeremonyCommand, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyEndReason, CeremonyLifecyclePhase, CeremonyTimeout, DurationMs, LifecycleReason,
    MaxParallel, StateTimeout, StepClaimFence, StepOutput, StepResult, StepStatus,
};

use super::fixture::{at, definition, lease, opened, readiness, role, state, step, OPENED_AT};

fn apply(instance: &mut CeremonyInstance, events: &[CeremonyEvent]) {
    for event in events {
        instance.apply(event);
    }
}

fn claim_plan(instance: &mut CeremonyInstance) -> StepClaimFence {
    let definition = definition();
    let events = instance
        .decide(
            &CeremonyCommand::StartStep(StartStep {
                role_id: Some(role("facilitator")),
                step_id: step("plan"),
                lease: lease("accepted-before-pause", at(1)),
                now: at(1),
                max_parallel_ceiling: MaxParallel::SERVER_MAX,
            }),
            &definition,
        )
        .unwrap();
    apply(instance, &events);
    instance.step_claim_fence(&step("plan")).unwrap()
}

#[test]
fn pause_blocks_new_work_but_an_accepted_claim_can_complete() {
    let definition = definition();
    let mut instance = opened(&definition);
    let fence = claim_plan(&mut instance);

    let paused = instance
        .decide(
            &CeremonyCommand::PauseCeremony(PauseCeremony {
                reason: LifecycleReason::new("operator hold").unwrap(),
                now: at(2),
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &paused);
    assert_eq!(instance.lifecycle().phase(), CeremonyLifecyclePhase::Paused);

    let refused = instance
        .decide(
            &CeremonyCommand::StartStep(StartStep {
                role_id: Some(role("facilitator")),
                step_id: step("plan"),
                lease: lease("new-during-pause", at(3)),
                now: at(3),
                max_parallel_ceiling: MaxParallel::SERVER_MAX,
            }),
            &definition,
        )
        .unwrap_err();
    assert!(matches!(
        refused,
        DomainError::LifecycleRefused {
            operation: "start_step",
            phase: CeremonyLifecyclePhase::Paused
        }
    ));

    let completed = instance
        .decide(
            &CeremonyCommand::ApplyStepResult(ApplyStepResult {
                step_id: step("plan"),
                claim_fence: fence,
                result: StepResult::completed(readiness(true)).unwrap(),
                now: at(3),
            }),
            &definition,
        )
        .unwrap();
    assert!(matches!(
        completed.first(),
        Some(CeremonyEvent::StepCompleted(_))
    ));
    apply(&mut instance, &completed);
    assert_eq!(
        instance.step_record(&step("plan")).unwrap().status(),
        StepStatus::Completed
    );

    let resumed = instance
        .decide(
            &CeremonyCommand::ResumeCeremony(ResumeCeremony { now: at(4) }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &resumed);
    assert_eq!(
        instance.lifecycle().phase(),
        CeremonyLifecyclePhase::Running
    );
}

#[test]
fn exact_completion_after_cancel_is_an_idempotent_observation_only() {
    let definition = definition();
    let mut instance = opened(&definition);
    let fence = claim_plan(&mut instance);
    let cancelled = instance
        .decide(
            &CeremonyCommand::CancelCeremony(CancelCeremony {
                reason: LifecycleReason::new("stop requested").unwrap(),
                now: at(2),
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &cancelled);

    let command = CeremonyCommand::ApplyStepResult(ApplyStepResult {
        step_id: step("plan"),
        claim_fence: fence.clone(),
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        now: at(3),
    });
    let observed = instance.decide(&command, &definition).unwrap();
    assert!(matches!(
        observed.as_slice(),
        [CeremonyEvent::LateStepResultObserved(_)]
    ));
    apply(&mut instance, &observed);
    assert_eq!(
        instance.step_record(&step("plan")).unwrap().status(),
        StepStatus::InProgress
    );
    assert_eq!(instance.late_step_results().len(), 1);
    assert!(instance.decide(&command, &definition).unwrap().is_empty());

    let foreign = CeremonyCommand::ApplyStepResult(ApplyStepResult {
        step_id: step("plan"),
        claim_fence: StepClaimFence::new("0".repeat(64)).unwrap(),
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        now: at(4),
    });
    assert!(instance.decide(&foreign, &definition).is_err());
    assert_eq!(
        instance.lifecycle().end_reason(),
        Some(CeremonyEndReason::Cancelled)
    );
}

#[test]
fn paused_ceremony_still_observes_the_earliest_absolute_deadline() {
    let definition = definition()
        .with_ceremony_timeout(CeremonyTimeout::new(DurationMs::from_millis(120_000)).unwrap())
        .with_state_timeout(StateTimeout::new(DurationMs::from_millis(60_000)).unwrap());
    let mut instance = CeremonyInstance::start(
        made_core::value_objects::CeremonyId::new("deadline-order").unwrap(),
        &definition,
        made_core::value_objects::CeremonyContext::empty(),
        OPENED_AT,
    )
    .unwrap();
    let paused = instance
        .decide(
            &CeremonyCommand::PauseCeremony(PauseCeremony {
                reason: LifecycleReason::new("wait").unwrap(),
                now: at(0),
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &paused);

    let events = instance
        .decide(
            &CeremonyCommand::EnforceCeremonyDeadlines(EnforceCeremonyDeadlines { now: at(2) }),
            &definition,
        )
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [CeremonyEvent::StateDeadlineExceeded(_)]
    ));
    apply(&mut instance, &events);
    assert_eq!(
        instance.lifecycle().end_reason(),
        Some(CeremonyEndReason::StateDeadline)
    );
}

#[test]
fn historical_completion_fold_keeps_the_legacy_snapshot_shape() {
    let definition = definition();
    let opening = CeremonyInstance::decide_start(
        made_core::value_objects::CeremonyId::new("legacy-completed").unwrap(),
        &definition,
        made_core::value_objects::CeremonyContext::empty(),
        None,
        OPENED_AT,
    )
    .unwrap();
    let mut events = opening;
    events.push(CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
        final_state: state("done"),
        completed_at: at(1),
    }));
    let folded = CeremonyInstance::rehydrate(&events).unwrap();
    let encoded = serde_json::to_value(&folded).unwrap();
    assert!(encoded.get("lifecycle").is_none());

    let historical: CeremonyInstance = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(folded, historical);
    assert_eq!(serde_json::to_value(historical).unwrap(), encoded);
    assert_eq!(folded.lifecycle().phase(), CeremonyLifecyclePhase::Ended);
    assert_eq!(
        folded.lifecycle().end_reason(),
        Some(CeremonyEndReason::Completed)
    );
}
