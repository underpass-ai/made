use time::macros::datetime;

use crate::entities::ceremony_commands::StartStep;
use crate::entities::{CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{
    Attributes, CeremonyContext, CeremonyGuard, CeremonyId, CeremonyName, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, DurationMs,
    EventSchemaVersion, GuardCondition, GuardName, IdempotencyKey, LeaseOwnerId, MaxParallel,
    RepeatUntilCondition, RetryPolicy, RoleAction, RoleId, StateExecution, StateId,
    StepHandlerConfig, StepHandlerKind, StepId, StepIteration, StepLease, StepOutput,
    StepOutputField, StepRepeatPolicy, StepResult, StepStatus, TransitionTrigger,
};

fn role(raw: &str) -> RoleId {
    RoleId::new(raw).unwrap()
}

fn step(raw: &str) -> StepId {
    StepId::new(raw).unwrap()
}

fn definition(repeating_first_step: bool) -> CeremonyDefinition {
    let open = StateId::new("OPEN").unwrap();
    let done = StateId::new("DONE").unwrap();
    let mut first = CeremonyStep::new(
        step("step_a"),
        open.clone(),
        StepHandlerKind::new("handler").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    );
    if repeating_first_step {
        first = first.with_repeat_policy(StepRepeatPolicy::new(
            RepeatUntilCondition::output_field_equals(
                StepOutputField::new("ready").unwrap(),
                serde_json::json!(true),
            ),
            StepIteration::new(2).unwrap(),
        ));
    }
    let second = CeremonyStep::new(
        step("step_b"),
        open.clone(),
        StepHandlerKind::new("handler").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    );
    let guard = CeremonyGuard::new(
        GuardName::new("joined").unwrap(),
        GuardCondition::AllStepsCompleted,
    );
    let transition = CeremonyTransition::new(
        open.clone(),
        done.clone(),
        TransitionTrigger::new("finish").unwrap(),
        vec![guard.name().clone()],
    )
    .unwrap();
    let owner_a = CeremonyRole::new(
        role("OWNER_A"),
        [
            RoleAction::step(first.id().clone()),
            RoleAction::step(second.id().clone()),
        ],
    )
    .unwrap();
    let owner_b =
        CeremonyRole::new(role("CANONICAL_B"), [RoleAction::step(second.id().clone())]).unwrap();
    let shared = CeremonyRole::new(
        role("SHARED_X"),
        [
            RoleAction::step(first.id().clone()),
            RoleAction::step(second.id().clone()),
        ],
    )
    .unwrap();
    let driver = CeremonyRole::new(
        role("DRIVER"),
        [RoleAction::transition(transition.trigger().clone())],
    )
    .unwrap();
    CeremonyDefinition::new(
        CeremonyName::new("concurrent_static_roles").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(open).with_execution(StateExecution::Concurrent),
            CeremonyState::terminal(done),
        ],
        vec![transition],
        vec![first, second],
        vec![guard],
        vec![owner_a, owner_b, shared, driver],
    )
    .unwrap()
}

fn instance(definition: &CeremonyDefinition) -> CeremonyInstance {
    CeremonyInstance::start(
        CeremonyId::new("session").unwrap(),
        definition,
        CeremonyContext::empty(),
        datetime!(2026-09-18 12:00:00 UTC),
    )
    .unwrap()
}

fn start(
    instance: &CeremonyInstance,
    definition: &CeremonyDefinition,
    step_id: &str,
    requested_role: Option<RoleId>,
    key: &str,
) -> Result<Vec<CeremonyEvent>, DomainError> {
    instance.decide(
        &CeremonyCommand::StartStep(StartStep {
            role_id: requested_role,
            step_id: step(step_id),
            lease: StepLease::acquire(
                LeaseOwnerId::new("runner").unwrap(),
                IdempotencyKey::new(key).unwrap(),
                datetime!(2026-09-18 12:00:00 UTC),
                DurationMs::from_millis(60_000),
            )
            .unwrap(),
            now: datetime!(2026-09-18 12:00:00 UTC),
            max_parallel_ceiling: MaxParallel::SERVER_MAX,
        }),
        definition,
    )
}

fn started(events: &[CeremonyEvent]) -> &crate::entities::ceremony_events::StepStarted {
    let Some(CeremonyEvent::StepStarted(started)) = events.first() else {
        panic!("a start decision emits StepStarted");
    };
    started
}

#[test]
fn alternate_static_role_is_sealed_and_cannot_claim_two_concurrent_steps() {
    let definition = definition(false);
    let mut instance = instance(&definition);
    let events = start(
        &instance,
        &definition,
        "step_a",
        Some(role("SHARED_X")),
        "shared-a",
    )
    .unwrap();
    assert_eq!(started(&events).sealed_role, Some(role("SHARED_X")));
    assert_eq!(events[0].schema_version(), EventSchemaVersion::V3);
    instance.apply(&events[0]);

    let error = start(
        &instance,
        &definition,
        "step_b",
        Some(role("SHARED_X")),
        "shared-b",
    )
    .unwrap_err();
    assert!(matches!(
        error,
        DomainError::InvariantViolated {
            reason: "role is already assigned to another step in this state iteration"
        }
    ));
}

#[test]
fn canonical_unmarked_claim_reserves_its_role_against_an_alternate_claim() {
    let definition = definition(false);
    let mut instance = instance(&definition);
    let events = start(&instance, &definition, "step_a", None, "canonical-a").unwrap();
    assert_eq!(started(&events).sealed_role, None);
    assert_eq!(events[0].schema_version(), EventSchemaVersion::V2);
    instance.apply(&events[0]);

    assert!(start(
        &instance,
        &definition,
        "step_b",
        Some(role("OWNER_A")),
        "alternate-a",
    )
    .is_err());
}

#[test]
fn canonical_static_claims_keep_the_legacy_v2_shape() {
    let definition = definition(false);
    let mut instance = instance(&definition);
    for (step_id, key, expected) in [
        ("step_a", "canonical-a", "OWNER_A"),
        ("step_b", "canonical-b", "CANONICAL_B"),
    ] {
        let events = start(&instance, &definition, step_id, None, key).unwrap();
        assert_eq!(started(&events).started_by, role(expected));
        assert_eq!(started(&events).sealed_role, None);
        assert_eq!(events[0].schema_version(), EventSchemaVersion::V2);
        instance.apply(&events[0]);
    }
}

#[test]
fn pending_records_do_not_reserve_but_completed_repeat_history_does() {
    let definition = definition(true);
    let pending = instance(&definition);
    assert!(start(
        &pending,
        &definition,
        "step_b",
        Some(role("OWNER_A")),
        "pending-does-not-reserve",
    )
    .is_ok());

    let mut repeated = instance(&definition);
    let events = start(&repeated, &definition, "step_a", None, "repeat-a").unwrap();
    repeated.apply(&events[0]);
    let output = StepOutput::new(
        Attributes::new(std::collections::BTreeMap::from([(
            "ready".to_owned(),
            serde_json::json!(false),
        )]))
        .unwrap(),
    );
    repeated
        .apply_step_result(
            &definition,
            &step("step_a"),
            repeated.step_claim_fence(&step("step_a")).unwrap(),
            StepResult::completed(output).unwrap(),
            datetime!(2026-09-18 12:01:00 UTC),
        )
        .unwrap();
    assert_eq!(
        repeated.step_record(&step("step_a")).unwrap().status(),
        StepStatus::Pending
    );
    assert_eq!(repeated.step_record_history(&step("step_a")).len(), 1);
    assert!(start(
        &repeated,
        &definition,
        "step_b",
        Some(role("OWNER_A")),
        "history-reserves",
    )
    .is_err());
}
