use std::collections::{BTreeMap, BTreeSet};

use made_core::entities::ceremony_commands::ApplyStepResult;
use made_core::entities::ceremony_events::{CeremonyInstanceStarted, ContextWritten, StepStarted};
use made_core::entities::{CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, CeremonyContext, CeremonyGuard, CeremonyId, CeremonyName, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, ContextKey, ContextPatch,
    ContextWrites, DurationMs, DynamicRoleBinding, GuardCondition, GuardName, IdempotencyKey,
    LeaseOwnerId, MaxParallel, RetryPolicy, RoleAction, RoleId, StateExecution, StateId,
    StepAttempt, StepHandlerConfig, StepHandlerKind, StepId, StepIteration, StepLease, StepOutput,
    StepOutputField, StepResult, TransitionTrigger,
};
use time::{Duration, OffsetDateTime};

fn role(raw: &str) -> RoleId {
    RoleId::new(raw).unwrap()
}

fn step(raw: &str) -> StepId {
    StepId::new(raw).unwrap()
}

fn context(entries: &[(&str, serde_json::Value)]) -> CeremonyContext {
    CeremonyContext::new(
        Attributes::new(
            entries
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect(),
        )
        .unwrap(),
    )
}

fn lease(key: &str, now: OffsetDateTime) -> StepLease {
    StepLease::new(
        LeaseOwnerId::new("runner").unwrap(),
        IdempotencyKey::new(key).unwrap(),
        now,
        now + Duration::minutes(1),
    )
    .unwrap()
}

fn definition() -> CeremonyDefinition {
    let work = StateId::new("work").unwrap();
    let done = StateId::new("done").unwrap();
    let writer = step("writer");
    let dynamic = step("dynamic");
    let dynamic_peer = step("dynamic_peer");
    let dynamic_binding = DynamicRoleBinding::new(
        ContextKey::new("next_role").unwrap(),
        [role("A"), role("B"), role("C"), role("D"), role("X")],
    )
    .unwrap();
    let context_updates = ContextWrites::new(BTreeMap::from([(
        ContextKey::new("next_role").unwrap(),
        StepOutputField::new("assigned").unwrap(),
    )]));
    let steps = vec![
        CeremonyStep::new(
            writer.clone(),
            work.clone(),
            StepHandlerKind::new("host_callback").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::new(StepAttempt::new(3).unwrap(), DurationMs::ZERO),
            None,
        )
        .with_context_writes(context_updates),
        CeremonyStep::new(
            dynamic.clone(),
            work.clone(),
            StepHandlerKind::new("host_callback").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::new(StepAttempt::new(3).unwrap(), DurationMs::ZERO),
            None,
        )
        .with_dynamic_role_binding(dynamic_binding.clone()),
        CeremonyStep::new(
            dynamic_peer.clone(),
            work.clone(),
            StepHandlerKind::new("host_callback").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::new(StepAttempt::new(3).unwrap(), DurationMs::ZERO),
            None,
        )
        .with_dynamic_role_binding(dynamic_binding),
    ];
    let trigger = TransitionTrigger::new("finish").unwrap();
    let guard = GuardName::new("one_done").unwrap();
    let roles: Vec<_> = ["A", "B", "C", "D", "X"]
        .into_iter()
        .map(|id| {
            let mut actions = vec![if id == "A" || id == "X" {
                RoleAction::step(writer.clone())
            } else {
                RoleAction::step(dynamic.clone())
            }];
            if id != "A" {
                actions.push(RoleAction::step(dynamic_peer.clone()));
            }
            if id == "A" {
                actions.push(RoleAction::step(dynamic.clone()));
                actions.push(RoleAction::step(dynamic_peer.clone()));
            }
            if id == "X" {
                actions.push(RoleAction::step(dynamic.clone()));
            }
            if id == "A" {
                actions.push(RoleAction::transition(trigger.clone()));
            }
            CeremonyRole::new(role(id), actions).unwrap()
        })
        .collect();
    CeremonyDefinition::new(
        CeremonyName::new("dynamic_roles").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(work.clone()).with_execution(StateExecution::Concurrent),
            CeremonyState::terminal(done.clone()),
        ],
        vec![CeremonyTransition::new(work, done, trigger, vec![guard.clone()]).unwrap()],
        steps,
        vec![CeremonyGuard::new(guard, GuardCondition::AnyStepCompleted)],
        roles,
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(2).unwrap())
}

fn opened() -> CeremonyInstance {
    CeremonyInstance::start(
        CeremonyId::new("dynamic-1").unwrap(),
        &definition(),
        context(&[("next_role", serde_json::json!("B"))]),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}

fn two_state_definition() -> CeremonyDefinition {
    let first = StateId::new("first").unwrap();
    let second = StateId::new("second").unwrap();
    let done = StateId::new("done").unwrap();
    let first_step = step("first_step");
    let second_step = step("second_step");
    let binding =
        || DynamicRoleBinding::new(ContextKey::new("next_role").unwrap(), [role("B")]).unwrap();
    let make_step = |id: StepId, state: StateId| {
        CeremonyStep::new(
            id,
            state,
            StepHandlerKind::new("host_callback").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::new(StepAttempt::new(2).unwrap(), DurationMs::ZERO),
            None,
        )
        .with_dynamic_role_binding(binding())
    };
    let next = TransitionTrigger::new("next").unwrap();
    let finish = TransitionTrigger::new("finish").unwrap();
    let first_done = GuardName::new("first_done").unwrap();
    let second_done = GuardName::new("second_done").unwrap();
    CeremonyDefinition::new(
        CeremonyName::new("two_dynamic_states").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(first.clone()).with_execution(StateExecution::Concurrent),
            CeremonyState::intermediate(second.clone()).with_execution(StateExecution::Concurrent),
            CeremonyState::terminal(done.clone()),
        ],
        vec![
            CeremonyTransition::new(
                first.clone(),
                second.clone(),
                next.clone(),
                vec![first_done.clone()],
            )
            .unwrap(),
            CeremonyTransition::new(
                second.clone(),
                done,
                finish.clone(),
                vec![second_done.clone()],
            )
            .unwrap(),
        ],
        vec![
            make_step(first_step.clone(), first),
            make_step(second_step.clone(), second),
        ],
        vec![
            CeremonyGuard::new(first_done, GuardCondition::AnyStepCompleted),
            CeremonyGuard::new(second_done, GuardCondition::AnyStepCompleted),
        ],
        vec![CeremonyRole::new(
            role("B"),
            vec![
                RoleAction::step(first_step),
                RoleAction::step(second_step),
                RoleAction::transition(next),
                RoleAction::transition(finish),
            ],
        )
        .unwrap()],
    )
    .unwrap()
}

#[test]
fn dynamic_claim_seals_role_while_context_changes() {
    let definition = definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = opened();
    instance
        .start_step_as(
            &definition,
            &role("B"),
            &step("dynamic"),
            lease("b-1", now),
            now,
        )
        .unwrap();
    instance
        .start_step_as(
            &definition,
            &role("A"),
            &step("writer"),
            lease("a-1", now),
            now,
        )
        .unwrap();

    let output = StepOutput::new(
        Attributes::new(BTreeMap::from([(
            "assigned".to_owned(),
            serde_json::json!("C"),
        )]))
        .unwrap(),
    );
    let events = instance
        .decide(
            &CeremonyCommand::ApplyStepResult(ApplyStepResult {
                step_id: step("writer"),
                claim_fence: instance.step_claim_fence(&step("writer")).unwrap(),
                result: StepResult::completed(output).unwrap(),
                now,
            }),
            &definition,
        )
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            CeremonyEvent::StepCompleted(_),
            CeremonyEvent::ContextWritten(_)
        ]
    ));
    for event in &events {
        instance.apply(event);
    }
    assert_eq!(
        instance
            .context()
            .get(&ContextKey::new("next_role").unwrap()),
        Some(&serde_json::json!("C"))
    );

    let completion = instance
        .decide(
            &CeremonyCommand::ApplyStepResult(ApplyStepResult {
                step_id: step("dynamic"),
                claim_fence: instance.step_claim_fence(&step("dynamic")).unwrap(),
                result: StepResult::completed(StepOutput::empty()).unwrap(),
                now,
            }),
            &definition,
        )
        .unwrap();
    let CeremonyEvent::StepCompleted(completed) = &completion[0] else {
        panic!("dynamic completion emits StepCompleted first");
    };
    assert_eq!(completed.finished_by, role("B"));
}

#[test]
fn context_writes_validate_every_source_before_emitting_any_event() {
    let definition = definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = opened();
    instance
        .start_step_as(
            &definition,
            &role("A"),
            &step("writer"),
            lease("a-1", now),
            now,
        )
        .unwrap();
    let before = instance.clone();
    let decision = instance.decide(
        &CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step("writer"),
            claim_fence: instance.step_claim_fence(&step("writer")).unwrap(),
            result: StepResult::completed(StepOutput::empty()).unwrap(),
            now,
        }),
        &definition,
    );

    assert!(decision.is_err());
    assert_eq!(instance, before);
}

#[test]
fn dynamic_role_must_be_present_string_allowed_authorized_and_requested() {
    let definition = definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    for (value, requested) in [
        (serde_json::Value::Null, "B"),
        (serde_json::json!(3), "B"),
        (serde_json::json!("outside"), "outside"),
        (serde_json::json!("B"), "C"),
    ] {
        let mut instance = CeremonyInstance::start(
            CeremonyId::new(format!("dynamic-{requested}-{value}")).unwrap(),
            &definition,
            context(&[("next_role", value)]),
            now,
        )
        .unwrap();
        assert!(instance
            .start_step_as(
                &definition,
                &role(requested),
                &step("dynamic"),
                lease(&format!("{requested}-1"), now),
                now,
            )
            .is_err());
    }
}

#[test]
fn a_direct_claim_reports_the_dynamic_context_defect_before_claimability() {
    let definition = definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = CeremonyInstance::start(
        CeremonyId::new("missing-role-context").unwrap(),
        &definition,
        CeremonyContext::empty(),
        now,
    )
    .unwrap();
    let error = instance
        .start_step_as(
            &definition,
            &role("B"),
            &step("dynamic"),
            lease("missing-1", now),
            now,
        )
        .unwrap_err();
    assert_eq!(
        error,
        DomainError::NotFound {
            what: "ceremony_step.role_from.context_key"
        }
    );
}

#[test]
fn another_steps_dynamic_role_stays_reserved_after_lease_expiry() {
    let definition = definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = opened();
    instance
        .start_step_as(
            &definition,
            &role("B"),
            &step("dynamic"),
            lease("b-1", now),
            now,
        )
        .unwrap();

    assert!(
        instance
            .start_step_as(
                &definition,
                &role("B"),
                &step("dynamic_peer"),
                lease("b-2", now + Duration::minutes(1)),
                now + Duration::minutes(1),
            )
            .is_err(),
        "an expired lease still reserves its sealed role for another step"
    );
    assert!(
        instance
            .start_step_as(
                &definition,
                &role("B"),
                &step("dynamic"),
                lease("b-3", now + Duration::minutes(1)),
                now + Duration::minutes(1),
            )
            .is_ok(),
        "the same step may reclaim its expired attempt"
    );
}

#[test]
fn a_dynamic_claim_blocks_a_static_peer_claimed_by_a_non_default_role() {
    let definition = definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = CeremonyInstance::start(
        CeremonyId::new("mixed-static-dynamic").unwrap(),
        &definition,
        context(&[("next_role", serde_json::json!("X"))]),
        now,
    )
    .unwrap();
    instance
        .start_step_as(
            &definition,
            &role("X"),
            &step("dynamic"),
            lease("x-dynamic", now),
            now,
        )
        .unwrap();

    let error = instance
        .start_step_as(
            &definition,
            &role("X"),
            &step("writer"),
            lease("x-static", now),
            now,
        )
        .unwrap_err();
    assert!(error.to_string().contains("already assigned"), "{error}");
}

#[test]
fn an_explicit_free_static_role_is_not_overridden_by_the_default_claim_projection() {
    let definition = definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = CeremonyInstance::start(
        CeremonyId::new("explicit-static-role").unwrap(),
        &definition,
        context(&[("next_role", serde_json::json!("A"))]),
        now,
    )
    .unwrap();
    instance
        .start_step_as(
            &definition,
            &role("A"),
            &step("dynamic"),
            lease("dynamic-a", now),
            now,
        )
        .unwrap();
    instance
        .start_step_as(
            &definition,
            &role("X"),
            &step("writer"),
            lease("writer-x", now),
            now,
        )
        .expect("X is authorized and free even though default owner A is reserved");
    assert_eq!(
        instance
            .step_record(&step("writer"))
            .unwrap()
            .claimed_role(),
        Some(&role("X"))
    );
    let output = StepOutput::new(
        Attributes::new(BTreeMap::from([(
            "assigned".to_owned(),
            serde_json::json!("X"),
        )]))
        .unwrap(),
    );
    instance
        .apply_step_result(
            &definition,
            &step("writer"),
            instance.step_claim_fence(&step("writer")).unwrap(),
            StepResult::completed(output).unwrap(),
            now,
        )
        .unwrap();
    assert!(instance
        .start_step_as(
            &definition,
            &role("X"),
            &step("dynamic_peer"),
            lease("dynamic-x", now),
            now,
        )
        .is_err());
}

#[test]
fn a_role_used_in_an_earlier_concurrent_state_is_available_in_the_next_state() {
    let definition = two_state_definition();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = CeremonyInstance::start(
        CeremonyId::new("two-state-dynamic").unwrap(),
        &definition,
        context(&[("next_role", serde_json::json!("B"))]),
        now,
    )
    .unwrap();
    instance
        .start_step_as(
            &definition,
            &role("B"),
            &step("first_step"),
            lease("first-1", now),
            now,
        )
        .unwrap();
    instance
        .apply_step_result(
            &definition,
            &step("first_step"),
            instance.step_claim_fence(&step("first_step")).unwrap(),
            StepResult::completed(StepOutput::empty()).unwrap(),
            now,
        )
        .unwrap();
    instance
        .apply_transition_as(
            &definition,
            &role("B"),
            &TransitionTrigger::new("next").unwrap(),
            now,
        )
        .unwrap();

    assert!(instance
        .start_step_as(
            &definition,
            &role("B"),
            &step("second_step"),
            lease("second-1", now),
            now,
        )
        .is_ok());
}

#[test]
fn ordinary_static_step_started_events_keep_the_legacy_record_shape() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut instance = opened();
    let event = CeremonyEvent::StepStarted(StepStarted {
        state_visit: None,
        step_id: step("writer"),
        state_iteration: Some(made_core::value_objects::StateIteration::FIRST),
        iteration: StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        lease: lease("legacy-static", now),
        started_by: role("A"),
        role_from: None,
        sealed_role: None,
        deadline: None,
        budget_reservation_id: None,
        started_at: now,
    });
    instance.apply(&event);

    let record = instance.step_record(&step("writer")).unwrap();
    assert!(record.claimed_role().is_none());
    assert!(serde_json::to_value(record)
        .unwrap()
        .get("claimed_role")
        .is_none());
    let wire = serde_json::to_value(event).unwrap();
    assert!(wire.get("role_from").is_none());
    assert!(wire.get("sealed_role").is_none());
}

#[test]
fn legacy_step_records_omit_the_new_claimed_role_field() {
    let encoded =
        serde_json::to_value(made_core::value_objects::StepExecutionRecord::pending()).unwrap();
    assert!(encoded.get("claimed_role").is_none());
    let decoded: made_core::value_objects::StepExecutionRecord =
        serde_json::from_value(encoded).unwrap();
    assert!(decoded.claimed_role().is_none());
}

#[test]
fn a_literal_pre_p5_in_progress_snapshot_equals_its_full_legacy_fold() {
    let snapshot: CeremonyInstance = serde_json::from_str(include_str!(
        "fixtures/legacy_in_progress_instance_pre_p5.json"
    ))
    .unwrap();
    let at = time::macros::datetime!(2026-07-29 09:00:00 UTC);
    let events = [
        CeremonyEvent::CeremonyInstanceStarted(CeremonyInstanceStarted {
            ceremony_id: CeremonyId::new("legacy-active").unwrap(),
            definition_name: CeremonyName::new("legacy_static").unwrap(),
            definition_version: CeremonyVersion::v1(),
            initial_state: StateId::new("OPEN").unwrap(),
            step_ids: BTreeSet::from([step("draft")]),
            context: CeremonyContext::empty(),
            bound_definition: None,
            lineage: None,
            succession: None,
            budget_account_id: None,
            ceremony_deadline: None,
            state_deadline: None,
            created_at: at,
        }),
        CeremonyEvent::StepStarted(StepStarted {
            state_visit: None,
            step_id: step("draft"),
            state_iteration: None,
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            lease: StepLease::new(
                LeaseOwnerId::new("legacy-host").unwrap(),
                IdempotencyKey::new("legacy-claim-1").unwrap(),
                at,
                at + Duration::minutes(1),
            )
            .unwrap(),
            started_by: role("AUTHOR"),
            role_from: None,
            sealed_role: None,
            deadline: None,
            budget_reservation_id: None,
            started_at: at,
        }),
    ];

    let folded = CeremonyInstance::rehydrate(&events).unwrap();
    assert_eq!(folded, snapshot);
    assert_eq!(
        serde_json::to_value(folded).unwrap(),
        serde_json::to_value(snapshot).unwrap()
    );
}

#[test]
fn same_destination_context_writes_follow_stream_order() {
    let mut instance = opened();
    let key = ContextKey::new("next_role").unwrap();
    for value in ["C", "D"] {
        instance.apply(&CeremonyEvent::ContextWritten(ContextWritten {
            state_visit: None,
            step_id: step("writer"),
            state_iteration: made_core::value_objects::StateIteration::FIRST,
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            patch: ContextPatch::new(BTreeMap::from([(key.clone(), serde_json::json!(value))]))
                .unwrap(),
            written_at: OffsetDateTime::UNIX_EPOCH,
        }));
    }

    assert_eq!(instance.context().get(&key), Some(&serde_json::json!("D")));
}
