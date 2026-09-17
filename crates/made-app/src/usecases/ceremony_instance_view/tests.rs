use made_core::entities::{CeremonyDefinition, CeremonyInstance};
use made_core::value_objects::{
    Attributes, CeremonyContext, CeremonyGuard, CeremonyId, CeremonyName, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, DurationMs, GuardCondition,
    GuardName, IdempotencyKey, LeaseOwnerId, MaxParallel, RepeatUntilCondition, RetryPolicy,
    RoleAction, RoleId, StateExecution, StateId, StepAttempt, StepHandlerConfig, StepHandlerKind,
    StepId, StepIteration, StepLease, StepOutput, StepOutputField,
    StepRepeatExhaustedGuardCondition, StepRepeatPolicy, StepResult, StepStatus, TransitionTrigger,
};
use serde_json::json;
use time::{Duration, OffsetDateTime};

use super::CeremonyInstanceView;

#[test]
fn exhausted_repeat_exposes_the_only_remaining_human_guard() {
    let definition = definition();
    let step_id = StepId::new("review").unwrap();
    let role_id = RoleId::new("reviewer").unwrap();
    let mut instance = CeremonyInstance::start(
        CeremonyId::new("review-session").unwrap(),
        &definition,
        CeremonyContext::empty(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();

    for iteration in 1_i64..=3 {
        let now = OffsetDateTime::UNIX_EPOCH + Duration::seconds(iteration * 2);
        instance
            .start_step_as(&definition, &role_id, &step_id, lease(iteration, now), now)
            .unwrap();
        let output =
            StepOutput::new(Attributes::new([("ready".to_owned(), json!(false))].into()).unwrap());
        instance
            .apply_step_result(
                &definition,
                &step_id,
                StepResult::completed(output).unwrap(),
                now + Duration::seconds(1),
            )
            .unwrap();
    }

    let view = CeremonyInstanceView::project(&instance, &definition).unwrap();
    assert_eq!(
        view.waiting_for_human(),
        &[&GuardName::new("human_approved").unwrap()]
    );
}

#[test]
fn projection_and_decision_share_the_live_lease_barrier_and_expiry_clock() {
    let definition = concurrent_definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = CeremonyInstance::start(
        CeremonyId::new("projection-session").unwrap(),
        &definition,
        CeremonyContext::empty(),
        now,
    )
    .unwrap();
    instance
        .start_step(&definition, &StepId::new("a").unwrap(), lease(10, now), now)
        .unwrap();
    instance
        .start_step(&definition, &StepId::new("b").unwrap(), lease(11, now), now)
        .unwrap();
    instance
        .apply_step_result(
            &definition,
            &StepId::new("a").unwrap(),
            StepResult::completed(StepOutput::empty()).unwrap(),
            now,
        )
        .unwrap();

    let live = CeremonyInstanceView::project_at(
        &instance,
        &definition,
        now + Duration::seconds(30),
        MaxParallel::SERVER_MAX,
    )
    .unwrap();
    assert!(!live.transitions()[0].is_enabled());
    assert!(live.waiting_for_human().is_empty());
    assert!(instance
        .apply_transition(
            &definition,
            &TransitionTrigger::new("finish").unwrap(),
            now + Duration::seconds(30),
        )
        .is_err());

    let expired = CeremonyInstanceView::project_at(
        &instance,
        &definition,
        now + Duration::seconds(60),
        MaxParallel::SERVER_MAX,
    )
    .unwrap();
    assert!(!expired.transitions()[0].is_enabled());
    assert_eq!(
        expired.waiting_for_human(),
        &[&GuardName::new("human_approved").unwrap()]
    );
    assert_eq!(expired.claimable_step_ids(), &[&StepId::new("b").unwrap()]);
}

fn definition() -> CeremonyDefinition {
    let step_id = StepId::new("review").unwrap();
    let role_id = RoleId::new("reviewer").unwrap();
    let trigger = TransitionTrigger::new("review_completed").unwrap();
    let completion = CeremonyGuard::new(
        GuardName::new("review_done").unwrap(),
        GuardCondition::StepStatus {
            step_id: step_id.clone(),
            status: StepStatus::Completed,
        },
    );
    let exhausted = CeremonyGuard::new(
        GuardName::new("review_exhausted").unwrap(),
        GuardCondition::StepRepeatExhausted(StepRepeatExhaustedGuardCondition::new(
            step_id.clone(),
        )),
    );
    let human = CeremonyGuard::new(
        GuardName::new("human_approved").unwrap(),
        GuardCondition::HumanApproval,
    );
    let transition = CeremonyTransition::new(
        StateId::new("REVIEWING").unwrap(),
        StateId::new("DONE").unwrap(),
        trigger.clone(),
        vec![
            completion.name().clone(),
            exhausted.name().clone(),
            human.name().clone(),
        ],
    )
    .unwrap();
    let step = CeremonyStep::new(
        step_id.clone(),
        StateId::new("REVIEWING").unwrap(),
        StepHandlerKind::new("host_callback").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    )
    .with_repeat_policy(StepRepeatPolicy::new(
        RepeatUntilCondition::output_field_equals(
            StepOutputField::new("ready").unwrap(),
            json!(true),
        ),
        StepIteration::new(3).unwrap(),
    ));
    let role = CeremonyRole::new(
        role_id,
        vec![RoleAction::step(step_id), RoleAction::transition(trigger)],
    )
    .unwrap();

    CeremonyDefinition::new(
        CeremonyName::new("review_contract").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(StateId::new("REVIEWING").unwrap()),
            CeremonyState::terminal(StateId::new("DONE").unwrap()),
        ],
        vec![transition],
        vec![step],
        vec![completion, exhausted, human],
        vec![role],
    )
    .unwrap()
}

fn concurrent_definition() -> CeremonyDefinition {
    let work = StateId::new("WORK").unwrap();
    let done = StateId::new("DONE").unwrap();
    let trigger = TransitionTrigger::new("finish").unwrap();
    let join = CeremonyGuard::new(
        GuardName::new("one_done").unwrap(),
        GuardCondition::AnyStepCompleted,
    );
    let human = CeremonyGuard::new(
        GuardName::new("human_approved").unwrap(),
        GuardCondition::HumanApproval,
    );
    let mut steps = Vec::new();
    let mut roles = Vec::new();
    for id in ["a", "b"] {
        let step_id = StepId::new(id).unwrap();
        steps.push(CeremonyStep::new(
            step_id.clone(),
            work.clone(),
            StepHandlerKind::new("host_callback").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::new(StepAttempt::new(2).unwrap(), DurationMs::ZERO),
            None,
        ));
        let mut actions = vec![RoleAction::step(step_id)];
        if id == "a" {
            actions.push(RoleAction::transition(trigger.clone()));
        }
        roles.push(CeremonyRole::new(RoleId::new(format!("role_{id}")).unwrap(), actions).unwrap());
    }
    CeremonyDefinition::new(
        CeremonyName::new("concurrent_projection").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(work.clone()).with_execution(StateExecution::Concurrent),
            CeremonyState::terminal(done.clone()),
        ],
        vec![CeremonyTransition::new(
            work,
            done,
            trigger,
            vec![join.name().clone(), human.name().clone()],
        )
        .unwrap()],
        steps,
        vec![join, human],
        roles,
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(2).unwrap())
}

fn lease(iteration: i64, now: OffsetDateTime) -> StepLease {
    StepLease::acquire(
        LeaseOwnerId::new("worker").unwrap(),
        IdempotencyKey::new(format!("review-{iteration}")).unwrap(),
        now,
        DurationMs::from_millis(60_000),
    )
    .unwrap()
}
