use made_core::entities::{CeremonyDefinition, CeremonyInstance};
use made_core::value_objects::{
    CeremonyContext, CeremonyGuard, CeremonyId, CeremonyName, CeremonyRole, CeremonyState,
    CeremonyStep, CeremonyTransition, CeremonyVersion, DurationMs, GuardCondition, GuardName,
    IdempotencyKey, JoinStepCount, LeaseOwnerId, MaxParallel, RetryPolicy, RoleAction, RoleId,
    StateExecution, StateId, StepAttempt, StepExecutionRecord, StepHandlerConfig, StepHandlerKind,
    StepId, StepLease, StepOutput, StepResult, TransitionTrigger,
};
use time::{Duration, OffsetDateTime};

fn step_id(raw: &str) -> StepId {
    StepId::new(raw).unwrap()
}
fn state_id(raw: &str) -> StateId {
    StateId::new(raw).unwrap()
}
fn role_id(raw: &str) -> RoleId {
    RoleId::new(raw).unwrap()
}
fn trigger(raw: &str) -> TransitionTrigger {
    TransitionTrigger::new(raw).unwrap()
}

fn definition(max_parallel: u8) -> CeremonyDefinition {
    let steps = ["a", "b", "c"]
        .into_iter()
        .map(|id| {
            CeremonyStep::new(
                step_id(id),
                state_id("work"),
                StepHandlerKind::new("host_callback").unwrap(),
                StepHandlerConfig::empty(),
                RetryPolicy::new(StepAttempt::new(3).unwrap(), DurationMs::ZERO),
                None,
            )
        })
        .collect::<Vec<_>>();
    let roles = ["a", "b", "c"]
        .into_iter()
        .map(|id| {
            CeremonyRole::new(
                role_id(&format!("role_{id}")),
                if id == "a" {
                    vec![
                        RoleAction::step(step_id(id)),
                        RoleAction::transition(trigger("finish")),
                    ]
                } else {
                    vec![RoleAction::step(step_id(id))]
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    CeremonyDefinition::new(
        CeremonyName::new("parallel_review").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(state_id("work")).with_execution(StateExecution::Concurrent),
            CeremonyState::terminal(state_id("done")),
        ],
        vec![CeremonyTransition::new(
            state_id("work"),
            state_id("done"),
            trigger("finish"),
            vec![GuardName::new("one_done").unwrap()],
        )
        .unwrap()],
        steps,
        vec![CeremonyGuard::new(
            GuardName::new("one_done").unwrap(),
            GuardCondition::AnyStepCompleted,
        )],
        roles,
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(max_parallel).unwrap())
}

fn lease(key: &str, now: OffsetDateTime) -> StepLease {
    StepLease::new(
        LeaseOwnerId::new("runner").unwrap(),
        IdempotencyKey::new(key).unwrap(),
        now,
        now + Duration::minutes(5),
    )
    .unwrap()
}

fn opened(definition: &CeremonyDefinition) -> CeremonyInstance {
    CeremonyInstance::start(
        CeremonyId::new("parallel-1").unwrap(),
        definition,
        CeremonyContext::empty(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}

#[test]
fn concurrent_claims_list_every_alternative_until_capacity_is_full() {
    let definition = definition(2);
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = opened(&definition);

    assert_eq!(
        instance
            .claimable_step_ids_at(&definition, now, MaxParallel::SERVER_MAX)
            .unwrap(),
        vec![&step_id("a"), &step_id("b"), &step_id("c")]
    );
    instance
        .start_step(&definition, &step_id("a"), lease("a-1", now), now)
        .unwrap();
    assert_eq!(
        instance
            .claimable_step_ids_at(&definition, now, MaxParallel::SERVER_MAX)
            .unwrap(),
        vec![&step_id("b"), &step_id("c")]
    );
    instance
        .start_step(&definition, &step_id("c"), lease("c-1", now), now)
        .unwrap();
    assert!(instance
        .claimable_step_ids_at(&definition, now, MaxParallel::SERVER_MAX)
        .unwrap()
        .is_empty());
}

#[test]
fn host_ceiling_and_expiry_are_applied_to_the_same_claim_query() {
    let definition = definition(3);
    let now = OffsetDateTime::UNIX_EPOCH;
    let digest = definition.digest().unwrap();
    let mut instance = opened(&definition);
    instance
        .start_step(&definition, &step_id("a"), lease("a-1", now), now)
        .unwrap();

    assert!(instance
        .claimable_step_ids_at(&definition, now, MaxParallel::new(1).unwrap())
        .unwrap()
        .is_empty());
    assert_eq!(
        instance
            .claimable_step_ids_at(
                &definition,
                now + Duration::minutes(5),
                MaxParallel::new(1).unwrap()
            )
            .unwrap(),
        vec![&step_id("a"), &step_id("b"), &step_id("c")]
    );
    assert_eq!(
        definition.digest().unwrap(),
        digest,
        "runtime ceilings never alter definition identity"
    );
}

#[test]
fn early_join_waits_for_other_live_leases_but_not_expired_ones() {
    let definition = definition(2);
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = opened(&definition);
    instance
        .start_step(&definition, &step_id("a"), lease("a-1", now), now)
        .unwrap();
    instance
        .start_step(&definition, &step_id("b"), lease("b-1", now), now)
        .unwrap();
    instance
        .apply_step_result(
            &definition,
            &step_id("a"),
            StepResult::completed(StepOutput::empty()).unwrap(),
            now + Duration::minutes(1),
        )
        .unwrap();

    assert!(instance
        .apply_transition(&definition, &trigger("finish"), now + Duration::minutes(1))
        .is_err());
    assert_eq!(
        instance
            .apply_transition(&definition, &trigger("finish"), now + Duration::minutes(5))
            .unwrap(),
        state_id("done")
    );
}

#[test]
fn default_concurrency_fields_are_elided_and_round_trip_without_digest_drift() {
    let mut legacy = definition(3);
    legacy = legacy.with_max_parallel(MaxParallel::DEFAULT);
    let json = serde_json::to_string(&legacy).unwrap();
    assert!(!json.contains("max_parallel"));
    assert!(!json.contains("execution\":\"sequential"));
    let reopened: CeremonyDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(reopened.digest().unwrap(), legacy.digest().unwrap());
}

#[test]
fn joins_count_only_steps_from_the_transition_source_state() {
    let definition = definition(3);
    let transition = &definition.transitions()[0];
    let mut records = opened(&definition).step_records().clone();
    records.insert(
        step_id("already_completed_elsewhere"),
        StepExecutionRecord::pending()
            .with_result(StepResult::completed(StepOutput::empty()).unwrap()),
    );

    for condition in [
        GuardCondition::AnyStepCompleted,
        GuardCondition::StepsCompleted(JoinStepCount::new(1).unwrap()),
    ] {
        let guard = CeremonyGuard::new(GuardName::new("scoped_join").unwrap(), condition);
        assert!(!definition.guard_is_satisfied_for_transition(
            &guard,
            transition,
            &records,
            &CeremonyContext::empty(),
        ));
    }
}
