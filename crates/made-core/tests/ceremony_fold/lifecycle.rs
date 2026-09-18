use made_core::entities::ceremony_commands::{
    ApplyExecutionReceiptResult, ApplyStepResult, CancelCeremony, EnforceCeremonyDeadlines,
    PauseCeremony, ResumeCeremony, StartStep,
};
use made_core::entities::ceremony_events::CeremonyCompleted;
use made_core::entities::{CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyEndReason, CeremonyLifecyclePhase, CeremonyTimeout, DurationMs, ExecutionOperationId,
    ExecutionReceiptId, ExecutionReceiptLink, ExecutionReceiptLinkKind, LifecycleReason,
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
                budget_reservation_id: None,
            }),
            &definition,
        )
        .unwrap();
    apply(instance, &events);
    instance.step_claim_fence(&step("plan")).unwrap()
}

fn timed_definition_with_alternate() -> CeremonyDefinition {
    let mut value = serde_json::to_value(definition()).unwrap();
    value["steps"]["plan"]["timeout"] = serde_json::json!(60_000);
    value["roles"]["alternate"] = serde_json::json!({
        "id": "alternate",
        "allowed_actions": [{"kind": "step", "value": "plan"}]
    });
    serde_json::from_value(value).unwrap()
}

fn start_plan_as(
    instance: &mut CeremonyInstance,
    definition: &CeremonyDefinition,
    role_id: &str,
    key: &str,
    minute: i64,
) -> StepClaimFence {
    let events = instance
        .decide(
            &CeremonyCommand::StartStep(StartStep {
                role_id: Some(role(role_id)),
                step_id: step("plan"),
                lease: lease(key, at(minute)),
                now: at(minute),
                max_parallel_ceiling: MaxParallel::SERVER_MAX,
                budget_reservation_id: None,
            }),
            definition,
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
                budget_reservation_id: None,
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
fn pause_accepts_a_direct_receipt_but_refuses_a_new_adoption() {
    let definition = definition();
    for (kind, accepted) in [
        (ExecutionReceiptLinkKind::Direct, true),
        (ExecutionReceiptLinkKind::Adopted, false),
    ] {
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
        let record = instance.step_record(&step("plan")).unwrap();
        let operation_id = ExecutionOperationId::for_step(
            instance.id(),
            &step("plan"),
            record.state_visit(),
            record.state_iteration(),
            record.iteration(),
        );
        let producer_fence = match kind {
            ExecutionReceiptLinkKind::Direct => fence.clone(),
            ExecutionReceiptLinkKind::Adopted => StepClaimFence::new("9".repeat(64)).unwrap(),
        };
        let link = ExecutionReceiptLink::new(
            ExecutionReceiptId::for_operation(&operation_id),
            operation_id,
            producer_fence,
            fence.clone(),
            kind,
        )
        .unwrap();
        let result = instance.decide(
            &CeremonyCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
                step_id: step("plan"),
                claim_fence: fence,
                receipt_link: link,
                result: StepResult::completed(readiness(true)).unwrap(),
                now: at(3),
            }),
            &definition,
        );

        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result,
                Err(DomainError::LifecycleRefused {
                    operation: "adopt_execution_receipt",
                    ..
                })
            ));
        }
    }
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

#[test]
fn step_deadline_accepts_the_retired_fence_before_a_retry() {
    let definition = timed_definition_with_alternate();
    let mut instance = CeremonyInstance::start(
        made_core::value_objects::CeremonyId::new("late-before-retry").unwrap(),
        &definition,
        made_core::value_objects::CeremonyContext::empty(),
        OPENED_AT,
    )
    .unwrap();
    let fence = start_plan_as(&mut instance, &definition, "facilitator", "first", 1);
    let deadline = instance
        .decide(
            &CeremonyCommand::EnforceCeremonyDeadlines(EnforceCeremonyDeadlines { now: at(2) }),
            &definition,
        )
        .unwrap();
    assert!(matches!(
        deadline.as_slice(),
        [CeremonyEvent::StepDeadlineExceeded(_)]
    ));
    apply(&mut instance, &deadline);
    assert!(!instance.is_ended());

    let command = CeremonyCommand::ApplyStepResult(ApplyStepResult {
        step_id: step("plan"),
        claim_fence: fence,
        result: StepResult::completed(readiness(true)).unwrap(),
        now: at(3),
    });
    let late = instance.decide(&command, &definition).unwrap();
    let [CeremonyEvent::LateStepResultObserved(observed)] = late.as_slice() else {
        panic!("retired deadline fence must produce one late observation");
    };
    assert_eq!(observed.result.finished_by(), &role("facilitator"));
    apply(&mut instance, &late);
    assert!(instance.decide(&command, &definition).unwrap().is_empty());
    assert_eq!(
        instance.step_record(&step("plan")).unwrap().status(),
        StepStatus::Failed
    );
}

#[test]
fn retired_fence_keeps_its_actor_after_a_retry_changes_role() {
    let definition = timed_definition_with_alternate();
    let mut instance = CeremonyInstance::start(
        made_core::value_objects::CeremonyId::new("late-after-retry").unwrap(),
        &definition,
        made_core::value_objects::CeremonyContext::empty(),
        OPENED_AT,
    )
    .unwrap();
    let retired = start_plan_as(&mut instance, &definition, "facilitator", "first", 1);
    let deadline = instance
        .decide(
            &CeremonyCommand::EnforceCeremonyDeadlines(EnforceCeremonyDeadlines { now: at(2) }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &deadline);
    let current = start_plan_as(&mut instance, &definition, "alternate", "second", 3);
    assert_ne!(retired, current);

    let late = instance
        .decide(
            &CeremonyCommand::ApplyStepResult(ApplyStepResult {
                step_id: step("plan"),
                claim_fence: retired.clone(),
                result: StepResult::completed(readiness(true)).unwrap(),
                now: at(4),
            }),
            &definition,
        )
        .unwrap();
    let [CeremonyEvent::LateStepResultObserved(observed)] = late.as_slice() else {
        panic!("retired fence must remain observable after retry");
    };
    assert_eq!(observed.result.finished_by(), &role("facilitator"));
    apply(&mut instance, &late);
    assert_eq!(instance.step_claim_fence(&step("plan")).unwrap(), current);

    let foreign_step = CeremonyCommand::ApplyStepResult(ApplyStepResult {
        step_id: step("check"),
        claim_fence: retired,
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        now: at(5),
    });
    assert!(instance.decide(&foreign_step, &definition).is_err());
    assert_eq!(instance.late_step_results().len(), 1);
}
