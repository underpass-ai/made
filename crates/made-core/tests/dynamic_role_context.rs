use std::collections::BTreeMap;

use made_core::entities::ceremony_commands::ApplyStepResult;
use made_core::entities::{CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use made_core::value_objects::{
    Attributes, CeremonyContext, CeremonyGuard, CeremonyId, CeremonyName, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, ContextKey, ContextWrites,
    DurationMs, DynamicRoleBinding, GuardCondition, GuardName, IdempotencyKey, LeaseOwnerId,
    MaxParallel, RetryPolicy, RoleAction, RoleId, StateExecution, StateId, StepAttempt,
    StepHandlerConfig, StepHandlerKind, StepId, StepLease, StepOutput, StepOutputField, StepResult,
    TransitionTrigger,
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
        [role("B"), role("C"), role("D")],
    )
    .unwrap();
    let writes = ContextWrites::new(BTreeMap::from([(
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
        .with_context_writes(writes),
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
    let roles: Vec<_> = ["A", "B", "C", "D"]
        .into_iter()
        .map(|id| {
            let mut actions = vec![if id == "A" {
                RoleAction::step(writer.clone())
            } else {
                RoleAction::step(dynamic.clone())
            }];
            if id != "A" {
                actions.push(RoleAction::step(dynamic_peer.clone()));
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
fn legacy_step_records_omit_the_new_claimed_role_field() {
    let encoded =
        serde_json::to_value(made_core::value_objects::StepExecutionRecord::pending()).unwrap();
    assert!(encoded.get("claimed_role").is_none());
    let decoded: made_core::value_objects::StepExecutionRecord =
        serde_json::from_value(encoded).unwrap();
    assert!(decoded.claimed_role().is_none());
}
