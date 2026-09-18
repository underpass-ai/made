use std::collections::BTreeMap;

use made_core::entities::CeremonyDefinition;
use made_core::value_objects::{
    Attributes, CeremonyContext, CeremonyGuard, CeremonyName, CeremonyState, CeremonyStep,
    CeremonyTransition, CeremonyVersion, DurationMs, GuardCondition, GuardName, IdempotencyKey,
    LeaseOwnerId, OutputFieldGuardCondition, RepeatUntilCondition, RetryPolicy, StateId,
    StepAttempt, StepExecutionRecord, StepHandlerConfig, StepHandlerKind, StepId, StepIteration,
    StepLease, StepOutput, StepOutputField, StepRepeatExhaustedGuardCondition, StepRepeatPolicy,
    StepResult, StepStatus, TransitionTrigger,
};
use serde_json::{json, Value};
use time::OffsetDateTime;

fn step_id(raw: &str) -> StepId {
    StepId::new(raw).unwrap()
}

fn state_id(raw: &str) -> StateId {
    StateId::new(raw).unwrap()
}

fn guard_name(raw: &str) -> GuardName {
    GuardName::new(raw).unwrap()
}

fn step(raw_step: &str, raw_state: &str, repeats: bool) -> CeremonyStep {
    let step = CeremonyStep::new(
        step_id(raw_step),
        state_id(raw_state),
        StepHandlerKind::new("host_callback").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    );
    if repeats {
        step.with_repeat_policy(StepRepeatPolicy::new(
            RepeatUntilCondition::output_field_equals(
                StepOutputField::new("ready").unwrap(),
                json!(true),
            ),
            StepIteration::new(3).unwrap(),
        ))
    } else {
        step
    }
}

fn completed(
    iteration: u32,
    entries: impl IntoIterator<Item = (&'static str, Value)>,
) -> StepExecutionRecord {
    let output = StepOutput::new(
        Attributes::new(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
        .unwrap(),
    );
    StepExecutionRecord::pending_iteration(StepIteration::new(iteration).unwrap())
        .with_result(StepResult::completed(output).unwrap())
}

fn in_progress() -> StepExecutionRecord {
    let lease = StepLease::acquire(
        LeaseOwnerId::new("worker").unwrap(),
        IdempotencyKey::new("active-lease").unwrap(),
        OffsetDateTime::UNIX_EPOCH,
        DurationMs::from_millis(60_000),
    )
    .unwrap();
    StepExecutionRecord::pending().with_started(lease, StepAttempt::FIRST, None)
}

fn transition(from: &str, to: &str, guards: &[&str]) -> CeremonyTransition {
    CeremonyTransition::new(
        state_id(from),
        state_id(to),
        TransitionTrigger::new(format!("leave_{from}")).unwrap(),
        guards.iter().map(|name| guard_name(name)),
    )
    .unwrap()
}

fn definition(
    states: Vec<CeremonyState>,
    transitions: Vec<CeremonyTransition>,
    steps: Vec<CeremonyStep>,
    guards: Vec<CeremonyGuard>,
) -> CeremonyDefinition {
    CeremonyDefinition::new(
        CeremonyName::new("guard_contract").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        states,
        transitions,
        steps,
        guards,
        Vec::new(),
    )
    .unwrap()
}

#[test]
fn output_field_guard_compares_the_current_successful_output_as_exact_json() {
    let condition = OutputFieldGuardCondition::new(
        step_id("produce"),
        StepOutputField::new("answer").unwrap(),
        json!(true),
    );
    let guard = CeremonyGuard::new(
        guard_name("answer_matches"),
        GuardCondition::OutputField(condition),
    );
    let move_on = transition("work", "done", &["answer_matches"]);
    let definition = definition(
        vec![
            CeremonyState::initial(state_id("work")),
            CeremonyState::terminal(state_id("done")),
        ],
        vec![move_on.clone()],
        vec![step("produce", "work", false)],
        vec![guard],
    );

    for (output, expected) in [
        (completed(1, [("answer", json!(true))]), true),
        (completed(1, [("answer", json!("true"))]), false),
        (completed(1, [("answer", json!(1))]), false),
        (completed(1, [("answer", Value::Null)]), false),
        (completed(1, []), false),
    ] {
        let records = BTreeMap::from([(step_id("produce"), output)]);
        assert_eq!(
            definition.guards_are_satisfied(&move_on, &records, &CeremonyContext::empty()),
            expected
        );
    }
}

#[test]
fn output_field_guard_may_reference_a_declared_step_from_an_earlier_state() {
    let prior_output = CeremonyGuard::new(
        guard_name("prior_output"),
        GuardCondition::OutputField(OutputFieldGuardCondition::new(
            step_id("produce"),
            StepOutputField::new("answer").unwrap(),
            json!({"accepted": true}),
        )),
    );
    let current_done = CeremonyGuard::new(
        guard_name("current_done"),
        GuardCondition::StepStatus {
            step_id: step_id("review"),
            status: StepStatus::Completed,
        },
    );
    let leave_review = transition("reviewing", "done", &["current_done", "prior_output"]);
    let definition = definition(
        vec![
            CeremonyState::initial(state_id("producing")),
            CeremonyState::intermediate(state_id("reviewing")),
            CeremonyState::terminal(state_id("done")),
        ],
        vec![
            transition("producing", "reviewing", &[]),
            leave_review.clone(),
        ],
        vec![
            step("produce", "producing", false),
            step("review", "reviewing", false),
        ],
        vec![current_done, prior_output],
    );
    let records = BTreeMap::from([
        (
            step_id("produce"),
            completed(1, [("answer", json!({"accepted": true}))]),
        ),
        (step_id("review"), completed(1, [])),
    ]);

    assert!(definition.guards_are_satisfied(&leave_review, &records, &CeremonyContext::empty()));
}

#[test]
fn exhausted_guard_waives_only_its_own_repeat_on_that_transition() {
    let a_done = CeremonyGuard::new(
        guard_name("a_done"),
        GuardCondition::StepStatus {
            step_id: step_id("repeat_a"),
            status: StepStatus::Completed,
        },
    );
    let b_done = CeremonyGuard::new(
        guard_name("b_done"),
        GuardCondition::StepStatus {
            step_id: step_id("repeat_b"),
            status: StepStatus::Completed,
        },
    );
    let a_exhausted = CeremonyGuard::new(
        guard_name("a_exhausted"),
        GuardCondition::StepRepeatExhausted(StepRepeatExhaustedGuardCondition::new(step_id(
            "repeat_a",
        ))),
    );
    let leave = transition("working", "done", &["a_done", "b_done", "a_exhausted"]);
    let definition = definition(
        vec![
            CeremonyState::initial(state_id("working")),
            CeremonyState::terminal(state_id("done")),
        ],
        vec![leave.clone()],
        vec![
            step("repeat_a", "working", true),
            step("repeat_b", "working", true),
        ],
        vec![a_done, b_done, a_exhausted],
    );
    let both_exhausted = BTreeMap::from([
        (step_id("repeat_a"), completed(3, [("ready", json!(false))])),
        (step_id("repeat_b"), completed(3, [("ready", json!(false))])),
    ]);
    assert!(!definition.guards_are_satisfied(&leave, &both_exhausted, &CeremonyContext::empty()));

    let only_a_exhausted = BTreeMap::from([
        (step_id("repeat_a"), completed(3, [("ready", json!(false))])),
        (step_id("repeat_b"), completed(2, [("ready", json!(true))])),
    ]);
    assert!(definition.guards_are_satisfied(&leave, &only_a_exhausted, &CeremonyContext::empty()));
}

#[test]
fn exhausted_guard_does_not_waive_a_required_in_progress_step() {
    let a_done = CeremonyGuard::new(
        guard_name("a_done"),
        GuardCondition::StepStatus {
            step_id: step_id("repeat_a"),
            status: StepStatus::Completed,
        },
    );
    let b_done = CeremonyGuard::new(
        guard_name("b_done"),
        GuardCondition::StepStatus {
            step_id: step_id("plain_b"),
            status: StepStatus::Completed,
        },
    );
    let a_exhausted = CeremonyGuard::new(
        guard_name("a_exhausted"),
        GuardCondition::StepRepeatExhausted(StepRepeatExhaustedGuardCondition::new(step_id(
            "repeat_a",
        ))),
    );
    let leave = transition("working", "done", &["a_done", "b_done", "a_exhausted"]);
    let definition = definition(
        vec![
            CeremonyState::initial(state_id("working")),
            CeremonyState::terminal(state_id("done")),
        ],
        vec![leave.clone()],
        vec![
            step("repeat_a", "working", true),
            step("plain_b", "working", false),
        ],
        vec![a_done, b_done, a_exhausted],
    );
    let records = BTreeMap::from([
        (step_id("repeat_a"), completed(3, [("ready", json!(false))])),
        (step_id("plain_b"), in_progress()),
    ]);

    assert!(!definition.guards_are_satisfied(&leave, &records, &CeremonyContext::empty()));
}

#[test]
fn legacy_completion_guards_keep_repeat_semantics_for_prior_steps() {
    let prior_done = CeremonyGuard::new(
        guard_name("prior_done"),
        GuardCondition::StepStatus {
            step_id: step_id("prior_repeat"),
            status: StepStatus::Completed,
        },
    );
    let all_done = CeremonyGuard::new(guard_name("all_done"), GuardCondition::AllStepsCompleted);
    let leave = transition("current", "done", &["prior_done", "all_done"]);
    let definition = definition(
        vec![
            CeremonyState::initial(state_id("prior")),
            CeremonyState::intermediate(state_id("current")),
            CeremonyState::terminal(state_id("done")),
        ],
        vec![transition("prior", "current", &[]), leave.clone()],
        vec![
            step("prior_repeat", "prior", true),
            step("current_step", "current", false),
        ],
        vec![prior_done.clone(), all_done.clone()],
    );
    let records = BTreeMap::from([
        (
            step_id("prior_repeat"),
            completed(3, [("ready", json!(false))]),
        ),
        (step_id("current_step"), completed(1, [])),
    ]);

    for guard in [&prior_done, &all_done] {
        assert!(!definition.guard_is_satisfied_for_transition(
            guard,
            &leave,
            &records,
            &CeremonyContext::empty(),
        ));
    }
}

#[test]
fn exhausted_guard_is_false_before_the_cap_and_when_the_until_condition_holds() {
    let exhausted = CeremonyGuard::new(
        guard_name("exhausted"),
        GuardCondition::StepRepeatExhausted(StepRepeatExhaustedGuardCondition::new(step_id(
            "repeat_a",
        ))),
    );
    let leave = transition("working", "done", &["exhausted"]);
    let definition = definition(
        vec![
            CeremonyState::initial(state_id("working")),
            CeremonyState::terminal(state_id("done")),
        ],
        vec![leave.clone()],
        vec![step("repeat_a", "working", true)],
        vec![exhausted],
    );

    for record in [
        completed(2, [("ready", json!(false))]),
        completed(3, [("ready", json!(true))]),
    ] {
        assert!(!definition.guards_are_satisfied(
            &leave,
            &BTreeMap::from([(step_id("repeat_a"), record)]),
            &CeremonyContext::empty()
        ));
    }
}

#[test]
fn exhausted_guard_does_not_waive_an_unrelated_human_guard() {
    let completed_guard = CeremonyGuard::new(
        guard_name("repeat_done"),
        GuardCondition::StepStatus {
            step_id: step_id("repeat_a"),
            status: StepStatus::Completed,
        },
    );
    let exhausted = CeremonyGuard::new(
        guard_name("exhausted"),
        GuardCondition::StepRepeatExhausted(StepRepeatExhaustedGuardCondition::new(step_id(
            "repeat_a",
        ))),
    );
    let human = CeremonyGuard::new(guard_name("human_approved"), GuardCondition::HumanApproval);
    let leave = transition(
        "working",
        "done",
        &["repeat_done", "exhausted", "human_approved"],
    );
    let definition = definition(
        vec![
            CeremonyState::initial(state_id("working")),
            CeremonyState::terminal(state_id("done")),
        ],
        vec![leave.clone()],
        vec![step("repeat_a", "working", true)],
        vec![completed_guard, exhausted, human],
    );
    let records = BTreeMap::from([(step_id("repeat_a"), completed(3, [("ready", json!(false))]))]);

    assert!(!definition.guards_are_satisfied(&leave, &records, &CeremonyContext::empty()));
}

#[test]
fn exhausted_guard_requires_a_repeating_step_in_the_transition_source() {
    let exhausted = |step: &str| {
        CeremonyGuard::new(
            guard_name("exhausted"),
            GuardCondition::StepRepeatExhausted(StepRepeatExhaustedGuardCondition::new(step_id(
                step,
            ))),
        )
    };
    let states = || {
        vec![
            CeremonyState::initial(state_id("first")),
            CeremonyState::intermediate(state_id("second")),
            CeremonyState::terminal(state_id("done")),
        ]
    };

    let not_repeating = CeremonyDefinition::new(
        CeremonyName::new("not_repeating").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        states(),
        vec![transition("first", "done", &["exhausted"])],
        vec![step("plain", "first", false)],
        vec![exhausted("plain")],
        Vec::new(),
    )
    .unwrap_err();
    assert!(not_repeating
        .to_string()
        .contains("must reference a repeating step"));

    let wrong_source = CeremonyDefinition::new(
        CeremonyName::new("wrong_source").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        states(),
        vec![transition("second", "done", &["exhausted"])],
        vec![step("repeat_a", "first", true)],
        vec![exhausted("repeat_a")],
        Vec::new(),
    )
    .unwrap_err();
    assert!(wrong_source.to_string().contains("transition source state"));
}
