use made_core::entities::ceremony_commands::ApplyStepResult;
use made_core::entities::{
    CeremonyCommand, CeremonyDefinition, CeremonyDefinitionDraft, CeremonyEvent, CeremonyInstance,
    PublishedCeremonyDefinition,
};
use made_core::value_objects::{
    Attributes, CeremonyContext, CeremonyGuard, CeremonyId, CeremonyName, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, DurationMs, GuardCondition,
    GuardName, IdempotencyKey, JoinStepCount, LeaseOwnerId, MaxParallel, RetryPolicy, RoleAction,
    RoleId, StateExecution, StateId, StateIteration, StateRepeatPolicy, StateRepeatUntilCondition,
    StepAttempt, StepErrorMessage, StepExecutionRecord, StepHandlerConfig, StepHandlerKind, StepId,
    StepLease, StepOutput, StepOutputField, StepResult, TransitionTrigger,
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
    definition_with_steps(&["a", "b", "c"], max_parallel)
}

fn definition_with_steps(ids: &[&str], max_parallel: u8) -> CeremonyDefinition {
    let steps = ids
        .iter()
        .copied()
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
    let roles = ids
        .iter()
        .copied()
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

fn repeating_concurrent_definition() -> CeremonyDefinition {
    let base = definition(3);
    CeremonyDefinition::new(
        base.name().clone(),
        base.version().clone(),
        base.description().cloned(),
        base.inputs().values().cloned(),
        base.outputs().values().cloned(),
        vec![
            CeremonyState::initial(state_id("work"))
                .with_execution(StateExecution::Concurrent)
                .with_repeat_policy(StateRepeatPolicy::new(
                    StateIteration::new(2).unwrap(),
                    StateRepeatUntilCondition::new(
                        step_id("b"),
                        StepOutputField::new("ready").unwrap(),
                        serde_json::json!(true),
                    ),
                )),
            CeremonyState::terminal(state_id("done")),
        ],
        base.transitions().iter().cloned(),
        base.steps_in_declaration_order().cloned(),
        base.guards().values().cloned(),
        base.roles().values().cloned(),
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(3).unwrap())
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
fn concurrent_state_repeat_waits_for_all_work_and_opens_one_boundary() {
    let definition = repeating_concurrent_definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = opened(&definition);
    for id in ["a", "b", "c"] {
        instance
            .start_step(
                &definition,
                &step_id(id),
                lease(&format!("{id}-1"), now),
                now,
            )
            .unwrap();
    }

    instance
        .apply_step_result(
            &definition,
            &step_id("a"),
            StepResult::completed(StepOutput::empty()).unwrap(),
            now,
        )
        .unwrap();
    assert!(instance
        .apply_transition(&definition, &trigger("finish"), now)
        .is_err());

    let not_ready = StepOutput::new(
        Attributes::new(std::collections::BTreeMap::from([(
            "ready".to_owned(),
            serde_json::json!(false),
        )]))
        .unwrap(),
    );
    instance
        .apply_step_result(
            &definition,
            &step_id("b"),
            StepResult::completed(not_ready).unwrap(),
            now,
        )
        .unwrap();
    assert_eq!(instance.current_state_iteration(), StateIteration::FIRST);

    let boundary = instance
        .decide(
            &CeremonyCommand::ApplyStepResult(ApplyStepResult {
                step_id: step_id("c"),
                result: StepResult::completed(StepOutput::empty()).unwrap(),
                now,
            }),
            &definition,
        )
        .unwrap();
    assert_eq!(
        boundary
            .iter()
            .filter(|event| matches!(event, CeremonyEvent::StateIterationStarted(_)))
            .count(),
        1
    );
    for event in &boundary {
        instance.apply(event);
    }

    assert_eq!(instance.current_state_iteration().get(), 2);
    for id in ["a", "b", "c"] {
        let record = instance.step_record(&step_id(id)).unwrap();
        assert_eq!(record.state_iteration().get(), 2);
        assert!(record.status().is_executable());
        assert_eq!(instance.step_record_history(&step_id(id)).len(), 1);
    }
    assert!(instance
        .apply_transition(&definition, &trigger("finish"), now)
        .is_err());
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
fn a_pre_concurrency_definition_reopens_and_keeps_its_published_binding() {
    let legacy_source = include_str!("fixtures/legacy_definition_pre_concurrency.json");
    let legacy: CeremonyDefinition = serde_json::from_str(legacy_source).unwrap();
    assert_eq!(legacy.max_parallel(), MaxParallel::DEFAULT);
    assert_eq!(legacy.max_transitions(), None);
    assert_eq!(legacy.max_bounces(), None);
    assert_eq!(
        legacy.state(&state_id("work")).unwrap().execution(),
        StateExecution::Sequential
    );
    let digest = legacy.digest().unwrap();
    assert_eq!(
        digest.to_hex(),
        "13b375c0cc738e9c15304662a0734350ed1d2c42cb708567de94acb2839a170d"
    );
    let published = PublishedCeremonyDefinition::seal(legacy.clone()).unwrap();
    let bound = CeremonyInstance::start_bound(
        CeremonyId::new("legacy-bound").unwrap(),
        &published,
        CeremonyContext::empty(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();

    let persisted = serde_json::to_string(&legacy).unwrap();
    assert!(!persisted.contains("max_parallel"));
    assert!(!persisted.contains("max_transitions"));
    assert!(!persisted.contains("max_bounces"));
    assert!(!persisted.contains("execution"));
    let reopened: CeremonyDefinition = serde_json::from_str(&persisted).unwrap();

    assert_eq!(reopened.digest().unwrap(), digest);
    assert_eq!(bound.bound_definition(), Some(digest));
    assert_eq!(published.digest(), digest);
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

#[test]
fn a_lost_capacity_race_rechecks_all_candidates_against_the_winning_claim() {
    let definition = definition(2);
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut persisted = opened(&definition);
    persisted
        .start_step(&definition, &step_id("a"), lease("a-1", now), now)
        .unwrap();
    assert_eq!(
        persisted
            .claimable_step_ids_at(&definition, now, MaxParallel::SERVER_MAX)
            .unwrap(),
        vec![&step_id("b"), &step_id("c")]
    );

    let mut stale = persisted.clone();
    stale
        .start_step(&definition, &step_id("c"), lease("c-stale", now), now)
        .unwrap();
    persisted
        .start_step(&definition, &step_id("b"), lease("b-wins", now), now)
        .unwrap();

    assert!(persisted
        .start_step(&definition, &step_id("c"), lease("c-retry", now), now)
        .is_err());
    assert!(persisted
        .claimable_step_ids_at(&definition, now, MaxParallel::SERVER_MAX)
        .unwrap()
        .is_empty());
}

#[test]
fn exhausted_retries_remove_only_that_step_from_concurrent_claims() {
    let definition = definition(2);
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = opened(&definition);
    for attempt in 1..=3 {
        instance
            .start_step(
                &definition,
                &step_id("a"),
                lease(&format!("a-{attempt}"), now),
                now,
            )
            .unwrap();
        instance
            .apply_step_result(
                &definition,
                &step_id("a"),
                StepResult::failed(StepErrorMessage::new("retry").unwrap()).unwrap(),
                now,
            )
            .unwrap();
    }

    assert_eq!(
        instance
            .claimable_step_ids_at(&definition, now, MaxParallel::SERVER_MAX)
            .unwrap(),
        vec![&step_id("b"), &step_id("c")]
    );
}

#[test]
fn analysis_rejects_a_concurrent_state_without_an_outgoing_join() {
    let base = definition(3);
    let draft = CeremonyDefinitionDraft::new(
        base.name().clone(),
        base.version().clone(),
        base.description().cloned(),
        base.inputs().values().cloned(),
        base.outputs().values().cloned(),
        base.states().values().cloned(),
        Vec::new(),
        base.steps_in_declaration_order().cloned(),
        base.guards().values().cloned(),
        base.roles().values().cloned(),
    );

    assert!(draft.analyze().errors().any(|finding| {
        finding
            .defect()
            .to_string()
            .contains("requires an outgoing join guard")
    }));
}

#[test]
fn analysis_warns_when_a_concurrent_state_has_more_than_three_owners() {
    let report = definition_with_steps(&["a", "b", "c", "d"], 4).analyze();

    assert!(report.warnings().any(|finding| {
        finding
            .defect()
            .to_string()
            .contains("more than three role owners")
    }));
}
