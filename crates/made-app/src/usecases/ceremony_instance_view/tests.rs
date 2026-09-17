use made_core::entities::{CeremonyDefinition, CeremonyInstance};
use made_core::value_objects::{
    Attributes, CeremonyContext, CeremonyGuard, CeremonyId, CeremonyName, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, DurationMs, GuardCondition,
    GuardName, IdempotencyKey, LeaseOwnerId, RepeatUntilCondition, RetryPolicy, RoleAction, RoleId,
    StateId, StepHandlerConfig, StepHandlerKind, StepId, StepIteration, StepLease, StepOutput,
    StepOutputField, StepRepeatExhaustedGuardCondition, StepRepeatPolicy, StepResult, StepStatus,
    TransitionTrigger,
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

fn lease(iteration: i64, now: OffsetDateTime) -> StepLease {
    StepLease::acquire(
        LeaseOwnerId::new("worker").unwrap(),
        IdempotencyKey::new(format!("review-{iteration}")).unwrap(),
        now,
        DurationMs::from_millis(60_000),
    )
    .unwrap()
}
