use made_core::entities::protocol_commands::{
    ApplyExecutionReceiptResult, ApplyStepResult, CancelProtocol, EnforceProtocolDeadlines,
    PauseProtocol, ResumeProtocol, StartStep,
};
use made_core::entities::protocol_events::{ExecutionReceiptLinked, ProtocolCompleted};
use made_core::entities::{ProtocolCommand, ProtocolDefinition, ProtocolEvent, ProtocolInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    DurationMs, ExecutionOperationId, ExecutionReceiptId, ExecutionReceiptLink,
    ExecutionReceiptLinkKind, LifecycleReason, MaxParallel, ProtocolEndReason,
    ProtocolLifecyclePhase, ProtocolTimeout, StateTimeout, StepClaimFence, StepOutput, StepResult,
    StepStatus,
};

use super::fixture::{at, definition, lease, opened, readiness, role, state, step, OPENED_AT};

fn apply(instance: &mut ProtocolInstance, events: &[ProtocolEvent]) {
    for event in events {
        instance.apply(event);
    }
}

fn claim_plan(instance: &mut ProtocolInstance) -> StepClaimFence {
    let definition = definition();
    let events = instance
        .decide(
            &ProtocolCommand::StartStep(StartStep {
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

fn timed_definition_with_alternate() -> ProtocolDefinition {
    let mut value = serde_json::to_value(definition()).unwrap();
    value["steps"]["plan"]["timeout"] = serde_json::json!(60_000);
    value["roles"]["alternate"] = serde_json::json!({
        "id": "alternate",
        "allowed_actions": [{"kind": "step", "value": "plan"}]
    });
    serde_json::from_value(value).unwrap()
}

fn start_plan_as(
    instance: &mut ProtocolInstance,
    definition: &ProtocolDefinition,
    role_id: &str,
    key: &str,
    minute: i64,
) -> StepClaimFence {
    let events = instance
        .decide(
            &ProtocolCommand::StartStep(StartStep {
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
            &ProtocolCommand::PauseProtocol(PauseProtocol {
                reason: LifecycleReason::new("operator hold").unwrap(),
                now: at(2),
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &paused);
    assert_eq!(instance.lifecycle().phase(), ProtocolLifecyclePhase::Paused);

    let refused = instance
        .decide(
            &ProtocolCommand::StartStep(StartStep {
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
            phase: ProtocolLifecyclePhase::Paused
        }
    ));

    let completed = instance
        .decide(
            &ProtocolCommand::ApplyStepResult(ApplyStepResult {
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
        Some(ProtocolEvent::StepCompleted(_))
    ));
    apply(&mut instance, &completed);
    assert_eq!(
        instance.step_record(&step("plan")).unwrap().status(),
        StepStatus::Completed
    );

    let resumed = instance
        .decide(
            &ProtocolCommand::ResumeProtocol(ResumeProtocol { now: at(4) }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &resumed);
    assert_eq!(
        instance.lifecycle().phase(),
        ProtocolLifecyclePhase::Running
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
                &ProtocolCommand::PauseProtocol(PauseProtocol {
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
            &ProtocolCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
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
            &ProtocolCommand::CancelProtocol(CancelProtocol {
                reason: LifecycleReason::new("stop requested").unwrap(),
                now: at(2),
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &cancelled);

    let command = ProtocolCommand::ApplyStepResult(ApplyStepResult {
        step_id: step("plan"),
        claim_fence: fence.clone(),
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        now: at(3),
    });
    let observed = instance.decide(&command, &definition).unwrap();
    assert!(matches!(
        observed.as_slice(),
        [ProtocolEvent::LateStepResultObserved(_)]
    ));
    apply(&mut instance, &observed);
    assert_eq!(
        instance.step_record(&step("plan")).unwrap().status(),
        StepStatus::InProgress
    );
    assert_eq!(instance.late_step_results().len(), 1);
    assert!(instance.decide(&command, &definition).unwrap().is_empty());

    let foreign = ProtocolCommand::ApplyStepResult(ApplyStepResult {
        step_id: step("plan"),
        claim_fence: StepClaimFence::new("0".repeat(64)).unwrap(),
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        now: at(4),
    });
    assert!(instance.decide(&foreign, &definition).is_err());
    assert_eq!(
        instance.lifecycle().end_reason(),
        Some(ProtocolEndReason::Cancelled)
    );
}

#[test]
fn paused_protocol_still_observes_the_earliest_absolute_deadline() {
    let definition = definition()
        .with_protocol_timeout(ProtocolTimeout::new(DurationMs::from_millis(120_000)).unwrap())
        .with_state_timeout(StateTimeout::new(DurationMs::from_millis(60_000)).unwrap());
    let mut instance = ProtocolInstance::start(
        made_core::value_objects::ProtocolId::new("deadline-order").unwrap(),
        &definition,
        made_core::value_objects::ProtocolContext::empty(),
        OPENED_AT,
    )
    .unwrap();
    let paused = instance
        .decide(
            &ProtocolCommand::PauseProtocol(PauseProtocol {
                reason: LifecycleReason::new("wait").unwrap(),
                now: at(0),
            }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &paused);

    let events = instance
        .decide(
            &ProtocolCommand::EnforceProtocolDeadlines(EnforceProtocolDeadlines { now: at(2) }),
            &definition,
        )
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [ProtocolEvent::StateDeadlineExceeded(_)]
    ));
    apply(&mut instance, &events);
    assert_eq!(
        instance.lifecycle().end_reason(),
        Some(ProtocolEndReason::StateDeadline)
    );
}

#[test]
fn historical_completion_fold_keeps_the_legacy_snapshot_shape() {
    let definition = definition();
    let opening = ProtocolInstance::decide_start(
        made_core::value_objects::ProtocolId::new("legacy-completed").unwrap(),
        &definition,
        made_core::value_objects::ProtocolContext::empty(),
        None,
        OPENED_AT,
    )
    .unwrap();
    let mut events = opening;
    events.push(ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
        final_state: state("done"),
        completed_at: at(1),
    }));
    let folded = ProtocolInstance::rehydrate(&events).unwrap();
    let encoded = serde_json::to_value(&folded).unwrap();
    assert!(encoded.get("lifecycle").is_none());
    assert!(encoded.get("execution_receipt_adoptions").is_none());

    let historical: ProtocolInstance = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(folded, historical);
    assert_eq!(serde_json::to_value(historical).unwrap(), encoded);
    assert_eq!(folded.lifecycle().phase(), ProtocolLifecyclePhase::Ended);
    assert_eq!(
        folded.lifecycle().end_reason(),
        Some(ProtocolEndReason::Completed)
    );
}

#[test]
fn two_receipt_adoptions_survive_snapshot_roundtrip_and_tail_replay() {
    let definition = definition();
    let protocol_id = made_core::value_objects::ProtocolId::new("receipt-adoption-tail").unwrap();
    let mut events = ProtocolInstance::decide_start(
        protocol_id.clone(),
        &definition,
        made_core::value_objects::ProtocolContext::empty(),
        None,
        OPENED_AT,
    )
    .unwrap();
    let operation = ExecutionOperationId::for_step(
        &protocol_id,
        &step("plan"),
        made_core::value_objects::StateVisit::FIRST,
        made_core::value_objects::StateIteration::FIRST,
        made_core::value_objects::StepIteration::FIRST,
    );
    let receipt = ExecutionReceiptId::for_operation(&operation);
    let producer = StepClaimFence::new("1".repeat(64)).unwrap();
    let first_applied = StepClaimFence::new("2".repeat(64)).unwrap();
    let second_applied = StepClaimFence::new("3".repeat(64)).unwrap();
    let receipt_event = |applied: StepClaimFence, kind| {
        ProtocolEvent::ExecutionReceiptLinked(ExecutionReceiptLinked {
            step_id: step("plan"),
            link: ExecutionReceiptLink::new(
                receipt.clone(),
                operation.clone(),
                producer.clone(),
                applied,
                kind,
            )
            .unwrap(),
            linked_at: at(1),
        })
    };
    events.push(receipt_event(
        producer.clone(),
        ExecutionReceiptLinkKind::Direct,
    ));
    events.push(receipt_event(
        first_applied.clone(),
        ExecutionReceiptLinkKind::Adopted,
    ));
    let tail = receipt_event(second_applied.clone(), ExecutionReceiptLinkKind::Adopted);

    let mut full_events = events.clone();
    full_events.push(tail.clone());
    let full = ProtocolInstance::rehydrate(&full_events).unwrap();
    let snapshot = ProtocolInstance::rehydrate(&events).unwrap();
    let encoded = serde_json::to_value(snapshot).unwrap();
    let mut snapshot_tail: ProtocolInstance = serde_json::from_value(encoded).unwrap();
    snapshot_tail.apply(&tail);

    assert_eq!(snapshot_tail, full);
    assert!(full
        .execution_receipt_adoption(&operation, &first_applied)
        .is_some());
    assert!(full
        .execution_receipt_adoption(&operation, &second_applied)
        .is_some());
}

#[test]
fn a_first_adoption_still_rejects_an_operation_from_another_step() {
    let definition = timed_definition_with_alternate();
    let mut instance = ProtocolInstance::start(
        made_core::value_objects::ProtocolId::new("adoption-without-producer").unwrap(),
        &definition,
        made_core::value_objects::ProtocolContext::empty(),
        OPENED_AT,
    )
    .unwrap();
    let producer = start_plan_as(&mut instance, &definition, "facilitator", "first", 1);
    let record = instance.step_record(&step("plan")).unwrap();
    let operation_id = ExecutionOperationId::for_step(
        instance.id(),
        &step("check"),
        record.state_visit(),
        record.state_iteration(),
        record.iteration(),
    );
    let deadline = instance
        .decide(
            &ProtocolCommand::EnforceProtocolDeadlines(EnforceProtocolDeadlines { now: at(2) }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &deadline);
    let current = start_plan_as(&mut instance, &definition, "alternate", "second", 4);
    let adoption = ExecutionReceiptLink::new(
        ExecutionReceiptId::for_operation(&operation_id),
        operation_id,
        producer,
        current.clone(),
        ExecutionReceiptLinkKind::Adopted,
    )
    .unwrap();

    assert!(matches!(
        instance.decide(
            &ProtocolCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
                step_id: step("plan"),
                claim_fence: current,
                receipt_link: adoption,
                result: StepResult::completed(readiness(true)).unwrap(),
                now: at(5),
            }),
            &definition,
        ),
        Err(DomainError::InvariantViolated {
            reason: "execution receipt operation does not match the step coordinates"
        })
    ));
}

#[test]
fn step_deadline_accepts_the_retired_fence_before_a_retry() {
    let definition = timed_definition_with_alternate();
    let mut instance = ProtocolInstance::start(
        made_core::value_objects::ProtocolId::new("late-before-retry").unwrap(),
        &definition,
        made_core::value_objects::ProtocolContext::empty(),
        OPENED_AT,
    )
    .unwrap();
    let fence = start_plan_as(&mut instance, &definition, "facilitator", "first", 1);
    let deadline = instance
        .decide(
            &ProtocolCommand::EnforceProtocolDeadlines(EnforceProtocolDeadlines { now: at(2) }),
            &definition,
        )
        .unwrap();
    assert!(matches!(
        deadline.as_slice(),
        [ProtocolEvent::StepDeadlineExceeded(_)]
    ));
    apply(&mut instance, &deadline);
    assert!(!instance.is_ended());

    let command = ProtocolCommand::ApplyStepResult(ApplyStepResult {
        step_id: step("plan"),
        claim_fence: fence,
        result: StepResult::completed(readiness(true)).unwrap(),
        now: at(3),
    });
    let late = instance.decide(&command, &definition).unwrap();
    let [ProtocolEvent::LateStepResultObserved(observed)] = late.as_slice() else {
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
    let mut instance = ProtocolInstance::start(
        made_core::value_objects::ProtocolId::new("late-after-retry").unwrap(),
        &definition,
        made_core::value_objects::ProtocolContext::empty(),
        OPENED_AT,
    )
    .unwrap();
    let retired = start_plan_as(&mut instance, &definition, "facilitator", "first", 1);
    let deadline = instance
        .decide(
            &ProtocolCommand::EnforceProtocolDeadlines(EnforceProtocolDeadlines { now: at(2) }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &deadline);
    let current = start_plan_as(&mut instance, &definition, "alternate", "second", 3);
    assert_ne!(retired, current);

    let late = instance
        .decide(
            &ProtocolCommand::ApplyStepResult(ApplyStepResult {
                step_id: step("plan"),
                claim_fence: retired.clone(),
                result: StepResult::completed(readiness(true)).unwrap(),
                now: at(4),
            }),
            &definition,
        )
        .unwrap();
    let [ProtocolEvent::LateStepResultObserved(observed)] = late.as_slice() else {
        panic!("retired fence must remain observable after retry");
    };
    assert_eq!(observed.result.finished_by(), &role("facilitator"));
    apply(&mut instance, &late);
    assert_eq!(instance.step_claim_fence(&step("plan")).unwrap(), current);

    let foreign_step = ProtocolCommand::ApplyStepResult(ApplyStepResult {
        step_id: step("check"),
        claim_fence: retired,
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        now: at(5),
    });
    assert!(instance.decide(&foreign_step, &definition).is_err());
    assert_eq!(instance.late_step_results().len(), 1);
}

fn late_receipt_with_retry() -> (
    ProtocolDefinition,
    ProtocolInstance,
    ExecutionOperationId,
    ExecutionReceiptLink,
    StepClaimFence,
    StepClaimFence,
) {
    let definition = timed_definition_with_alternate();
    let mut instance = ProtocolInstance::start(
        made_core::value_objects::ProtocolId::new("late-receipt-adoption").unwrap(),
        &definition,
        made_core::value_objects::ProtocolContext::empty(),
        OPENED_AT,
    )
    .unwrap();
    let retired = start_plan_as(&mut instance, &definition, "facilitator", "first", 1);
    let record = instance.step_record(&step("plan")).unwrap();
    let operation_id = ExecutionOperationId::for_step(
        instance.id(),
        &step("plan"),
        record.state_visit(),
        record.state_iteration(),
        record.iteration(),
    );
    let receipt_id = ExecutionReceiptId::for_operation(&operation_id);
    let deadline = instance
        .decide(
            &ProtocolCommand::EnforceProtocolDeadlines(EnforceProtocolDeadlines { now: at(2) }),
            &definition,
        )
        .unwrap();
    apply(&mut instance, &deadline);

    let original = ExecutionReceiptLink::new(
        receipt_id.clone(),
        operation_id.clone(),
        retired.clone(),
        retired.clone(),
        ExecutionReceiptLinkKind::Direct,
    )
    .unwrap();
    let late = instance
        .decide(
            &ProtocolCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
                step_id: step("plan"),
                claim_fence: retired.clone(),
                receipt_link: original.clone(),
                result: StepResult::completed(readiness(true)).unwrap(),
                now: at(3),
            }),
            &definition,
        )
        .unwrap();
    assert!(matches!(
        late.as_slice(),
        [
            ProtocolEvent::ExecutionReceiptLinked(_),
            ProtocolEvent::LateStepResultObserved(_)
        ]
    ));
    apply(&mut instance, &late);

    let current = start_plan_as(&mut instance, &definition, "alternate", "second", 4);
    (
        definition,
        instance,
        operation_id,
        original,
        retired,
        current,
    )
}

#[test]
fn a_recovered_receipt_can_be_adopted_before_any_direct_link_exists() {
    let definition = definition();
    let mut instance = opened(&definition);
    let producer = claim_plan(&mut instance);
    let current = start_plan_as(&mut instance, &definition, "facilitator", "recovery", 7);
    assert_ne!(producer, current);
    let record = instance.step_record(&step("plan")).unwrap();
    let operation = ExecutionOperationId::for_step(
        instance.id(),
        &step("plan"),
        record.state_visit(),
        record.state_iteration(),
        record.iteration(),
    );
    assert!(instance.execution_receipt_link(&operation).is_none());
    let adoption = ExecutionReceiptLink::new(
        ExecutionReceiptId::for_operation(&operation),
        operation.clone(),
        producer,
        current.clone(),
        ExecutionReceiptLinkKind::Adopted,
    )
    .unwrap();
    let command = ProtocolCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
        step_id: step("plan"),
        claim_fence: current,
        receipt_link: adoption.clone(),
        result: StepResult::completed(readiness(true)).unwrap(),
        now: at(8),
    });
    let events = instance.decide(&command, &definition).unwrap();
    assert!(matches!(
        events.as_slice(),
        [ProtocolEvent::ExecutionReceiptLinked(linked), ProtocolEvent::StepCompleted(_)]
            if linked.link == adoption
    ));
    apply(&mut instance, &events);
    assert_eq!(instance.execution_receipt_link(&operation), Some(&adoption));
    assert!(instance.execution_receipt_adoptions().is_empty());
    let reopened: ProtocolInstance =
        serde_json::from_slice(&serde_json::to_vec(&instance).unwrap()).unwrap();
    assert_eq!(reopened, instance);
    assert!(reopened.decide(&command, &definition).unwrap().is_empty());
}

#[test]
fn a_late_receipt_keeps_its_original_link_and_records_one_explicit_adoption() {
    let (definition, mut instance, operation_id, original, retired, current) =
        late_receipt_with_retry();
    let adoption = ExecutionReceiptLink::new(
        ExecutionReceiptId::for_operation(&operation_id),
        operation_id.clone(),
        retired,
        current.clone(),
        ExecutionReceiptLinkKind::Adopted,
    )
    .unwrap();
    let command = ProtocolCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
        step_id: step("plan"),
        claim_fence: current.clone(),
        receipt_link: adoption.clone(),
        result: StepResult::completed(readiness(true)).unwrap(),
        now: at(5),
    });
    let adopted = instance.decide(&command, &definition).unwrap();
    assert!(matches!(
        adopted.as_slice(),
        [
            ProtocolEvent::ExecutionReceiptLinked(_),
            ProtocolEvent::StepCompleted(_)
        ]
    ));
    apply(&mut instance, &adopted);

    assert_eq!(
        instance.execution_receipt_link(&operation_id),
        Some(&original)
    );
    assert_eq!(
        instance.execution_receipt_adoption(&operation_id, &current),
        Some(&adoption)
    );
    assert!(instance.decide(&command, &definition).unwrap().is_empty());
}

#[test]
fn an_adoption_rejects_a_contradictory_direct_link_or_producer_fence() {
    let (definition, instance, operation_id, _, _, current) = late_receipt_with_retry();
    let contradictory_direct = ExecutionReceiptLink::new(
        ExecutionReceiptId::for_operation(&operation_id),
        operation_id.clone(),
        current.clone(),
        current.clone(),
        ExecutionReceiptLinkKind::Direct,
    )
    .unwrap();
    assert!(matches!(
        instance.decide(
            &ProtocolCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
                step_id: step("plan"),
                claim_fence: current.clone(),
                receipt_link: contradictory_direct,
                result: StepResult::completed(readiness(true)).unwrap(),
                now: at(5),
            }),
            &definition,
        ),
        Err(DomainError::Conflict {
            what: "execution_receipt_link"
        })
    ));
    let foreign_producer = StepClaimFence::new("8".repeat(64)).unwrap();
    let wrong_receipt = ExecutionReceiptLink::new(
        ExecutionReceiptId::for_operation(&operation_id),
        operation_id.clone(),
        foreign_producer,
        current.clone(),
        ExecutionReceiptLinkKind::Adopted,
    )
    .unwrap();
    assert!(matches!(
        instance.decide(
            &ProtocolCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
                step_id: step("plan"),
                claim_fence: current.clone(),
                receipt_link: wrong_receipt,
                result: StepResult::completed(readiness(true)).unwrap(),
                now: at(5),
            }),
            &definition,
        ),
        Err(DomainError::Conflict {
            what: "execution_receipt_adoption"
        })
    ));
}
