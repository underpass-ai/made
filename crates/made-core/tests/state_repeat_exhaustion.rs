//! What a protocol does when a repeating state runs out of iterations.
//!
//! Without a routed exit the instance stops in the repeating state: the
//! state's work is complete, its `until` condition is false, no further
//! iteration starts and every declared transition is refused. A
//! `state_repeat_exhausted` guard declares the one transition that may
//! leave the state in that situation — for example to a terminal
//! `EXHAUSTED` — without weakening the transitions that require the
//! `until` condition.

use std::collections::BTreeMap;

use made_core::entities::ProtocolDefinition;
use made_core::entities::ProtocolInstance;
use made_core::value_objects::{
    Attributes, GuardCondition, GuardName, IdempotencyKey, LeaseOwnerId, MaxParallel,
    OutputFieldGuardCondition, ProtocolContext, ProtocolGuard, ProtocolId, ProtocolName,
    ProtocolRole, ProtocolState, ProtocolStep, ProtocolTransition, ProtocolVersion, RetryPolicy,
    RoleAction, RoleId, StateId, StateIteration, StateRepeatExhaustedGuardCondition,
    StateRepeatPolicy, StateRepeatUntilCondition, StepHandlerConfig, StepHandlerKind, StepId,
    StepLease, StepOutput, StepOutputField, StepResult, StepStatus, TransitionTrigger,
};
use serde_json::json;
use time::{Duration, OffsetDateTime};

const CYCLE: &str = "REVIEW_CYCLE";
const ROUNDS: u32 = 4;

fn step_id(raw: &str) -> StepId {
    StepId::new(raw).unwrap()
}

fn state_id(raw: &str) -> StateId {
    StateId::new(raw).unwrap()
}

fn guard_name(raw: &str) -> GuardName {
    GuardName::new(raw).unwrap()
}

fn trigger(raw: &str) -> TransitionTrigger {
    TransitionTrigger::new(raw).unwrap()
}

fn step(raw: &str) -> ProtocolStep {
    ProtocolStep::new(
        step_id(raw),
        state_id(CYCLE),
        StepHandlerKind::new("host_callback").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    )
}

fn exhausted_guard(state: &str) -> ProtocolGuard {
    ProtocolGuard::new(
        guard_name("review_cycle_repeat_exhausted"),
        GuardCondition::StateRepeatExhausted(StateRepeatExhaustedGuardCondition::new(state_id(
            state,
        ))),
    )
}

/// `REVIEW_CYCLE` repeats up to four times until `outcome` is
/// `approved`; `approve` leaves on approval. With `routed`, a second
/// transition leaves to the terminal `EXHAUSTED` on exhaustion.
fn definition(routed: bool) -> ProtocolDefinition {
    let verdict_done = ProtocolGuard::new(
        guard_name("verdict_done"),
        GuardCondition::StepStatus {
            step_id: step_id("verdict"),
            status: StepStatus::Completed,
        },
    );
    let outcome_done = ProtocolGuard::new(
        guard_name("outcome_done"),
        GuardCondition::StepStatus {
            step_id: step_id("outcome"),
            status: StepStatus::Completed,
        },
    );
    let approved = ProtocolGuard::new(
        guard_name("approved"),
        GuardCondition::OutputField(OutputFieldGuardCondition::new(
            step_id("outcome"),
            StepOutputField::new("outcome").unwrap(),
            json!("approved"),
        )),
    );
    let mut states = vec![
        ProtocolState::initial(state_id(CYCLE)).with_repeat_policy(StateRepeatPolicy::new(
            StateIteration::new(ROUNDS).unwrap(),
            StateRepeatUntilCondition::new(
                step_id("outcome"),
                StepOutputField::new("outcome").unwrap(),
                json!("approved"),
            ),
        )),
        ProtocolState::terminal(state_id("COMPLETED")),
    ];
    let mut transitions = vec![ProtocolTransition::new(
        state_id(CYCLE),
        state_id("COMPLETED"),
        trigger("approve"),
        [
            guard_name("verdict_done"),
            guard_name("outcome_done"),
            guard_name("approved"),
        ],
    )
    .unwrap()];
    let mut guards = vec![verdict_done, outcome_done, approved];
    if routed {
        states.push(ProtocolState::terminal(state_id("EXHAUSTED")));
        transitions.push(
            ProtocolTransition::new(
                state_id(CYCLE),
                state_id("EXHAUSTED"),
                trigger("give_up"),
                [
                    guard_name("verdict_done"),
                    guard_name("outcome_done"),
                    guard_name("review_cycle_repeat_exhausted"),
                ],
            )
            .unwrap(),
        );
        guards.push(exhausted_guard(CYCLE));
    }
    let host = ProtocolRole::new(
        RoleId::new("host").unwrap(),
        [
            RoleAction::step(step_id("verdict")),
            RoleAction::step(step_id("outcome")),
            RoleAction::transition(trigger("approve")),
        ]
        .into_iter()
        .chain(routed.then(|| RoleAction::transition(trigger("give_up")))),
    )
    .unwrap();
    ProtocolDefinition::new(
        ProtocolName::new("pr_review").unwrap(),
        ProtocolVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        states,
        transitions,
        vec![step("verdict"), step("outcome")],
        guards,
        [host],
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(1).unwrap())
}

fn outcome(value: &str) -> StepOutput {
    StepOutput::new(
        Attributes::new(BTreeMap::from([("outcome".to_owned(), json!(value))])).unwrap(),
    )
}

fn at(minutes: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::minutes(minutes)
}

/// Claim and complete both steps of the current iteration.
fn round(
    instance: &mut ProtocolInstance,
    definition: &ProtocolDefinition,
    number: u32,
    verdict: &str,
) {
    for (id, output) in [
        ("verdict", StepOutput::empty()),
        ("outcome", outcome(verdict)),
    ] {
        let now = at(i64::from(number) * 10);
        let lease = StepLease::new(
            LeaseOwnerId::new("host").unwrap(),
            IdempotencyKey::new(format!("{id}-{number}")).unwrap(),
            now,
            now + Duration::minutes(5),
        )
        .unwrap();
        instance
            .start_step(definition, &step_id(id), lease, now)
            .unwrap();
        instance
            .apply_step_result(
                definition,
                &step_id(id),
                instance.step_claim_fence(&step_id(id)).unwrap(),
                StepResult::completed(output).unwrap(),
                now,
            )
            .unwrap();
    }
}

fn opened(definition: &ProtocolDefinition) -> ProtocolInstance {
    ProtocolInstance::start(
        ProtocolId::new("pr-review-1").unwrap(),
        definition,
        ProtocolContext::empty(),
        at(0),
    )
    .unwrap()
}

fn exhaust(definition: &ProtocolDefinition) -> ProtocolInstance {
    let mut instance = opened(definition);
    for number in 1..=ROUNDS {
        round(&mut instance, definition, number, "changes_requested");
    }
    instance
}

#[test]
fn without_a_routed_exit_an_exhausted_state_repeat_leaves_the_instance_stuck() {
    let definition = definition(false);
    let mut instance = exhaust(&definition);

    assert_eq!(instance.current_state(), &state_id(CYCLE));
    assert_eq!(instance.current_state_iteration().get(), ROUNDS);
    assert!(instance.state_work_is_complete(&definition));
    assert!(instance.state_repeat_limit_reached(&definition));
    assert!(instance.state_repeat_exhaustion_is_unrouted(&definition));
    assert!(!instance.is_terminal(&definition));
    assert!(
        instance
            .claimable_step_ids_at(&definition, at(100), MaxParallel::SERVER_MAX)
            .unwrap()
            .is_empty(),
        "no fifth iteration starts"
    );
    let refusal = instance
        .apply_transition(&definition, &trigger("approve"), at(100))
        .unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("protocol state repeat condition is not satisfied"),
        "{refusal}"
    );
    assert_eq!(instance.current_state(), &state_id(CYCLE));
}

#[test]
fn a_routed_exit_leaves_an_exhausted_state_repeat_for_its_terminal() {
    let definition = definition(true);
    let mut instance = opened(&definition);
    for number in 1..ROUNDS {
        round(&mut instance, &definition, number, "changes_requested");
        assert!(
            instance
                .apply_transition(&definition, &trigger("give_up"), at(99))
                .is_err(),
            "round {number} of {ROUNDS} is not exhaustion"
        );
    }
    round(&mut instance, &definition, ROUNDS, "changes_requested");

    assert!(instance.state_repeat_limit_reached(&definition));
    assert!(!instance.state_repeat_exhaustion_is_unrouted(&definition));
    assert!(instance
        .apply_transition(&definition, &trigger("approve"), at(100))
        .is_err());
    let exhausted = instance
        .apply_transition(&definition, &trigger("give_up"), at(100))
        .unwrap();
    assert_eq!(exhausted, state_id("EXHAUSTED"));
    assert!(instance.is_terminal(&definition));
}

#[test]
fn a_routed_exit_is_refused_once_the_until_condition_holds() {
    let definition = definition(true);
    let mut instance = opened(&definition);
    round(&mut instance, &definition, 1, "changes_requested");
    round(&mut instance, &definition, 2, "approved");

    assert!(!instance.state_repeat_limit_reached(&definition));
    assert!(instance
        .apply_transition(&definition, &trigger("give_up"), at(100))
        .is_err());
    assert_eq!(
        instance
            .apply_transition(&definition, &trigger("approve"), at(100))
            .unwrap(),
        state_id("COMPLETED")
    );
}

#[test]
fn the_guard_must_name_a_repeating_state_and_the_transition_source() {
    let base = definition(true);
    let rebuilt =
        |states: Vec<ProtocolState>, extra: Option<ProtocolTransition>, guard: ProtocolGuard| {
            let replaced = guard.name().clone();
            let guards = base
                .guards()
                .values()
                .filter(|existing| existing.name() != &replaced)
                .cloned()
                .chain([guard]);
            ProtocolDefinition::new(
                base.name().clone(),
                base.version().clone(),
                None,
                Vec::new(),
                Vec::new(),
                states,
                base.transitions().iter().cloned().chain(extra),
                base.steps_in_declaration_order().cloned(),
                guards,
                base.roles().values().cloned(),
            )
        };
    let states = || base.states().values().cloned().collect::<Vec<_>>();

    let unknown = rebuilt(states(), None, exhausted_guard("NOWHERE")).unwrap_err();
    assert!(
        unknown.to_string().contains("protocol_guard.state"),
        "{unknown}"
    );

    let not_repeating = states()
        .into_iter()
        .map(|state| {
            if state.id() == &state_id(CYCLE) {
                ProtocolState::initial(state_id(CYCLE))
            } else {
                state
            }
        })
        .collect();
    let refusal = rebuilt(not_repeating, None, exhausted_guard(CYCLE)).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("must reference a repeating state"),
        "{refusal}"
    );

    let mut elsewhere = states();
    elsewhere.push(ProtocolState::intermediate(state_id("ELSEWHERE")));
    let from_elsewhere = ProtocolTransition::new(
        state_id("ELSEWHERE"),
        state_id("EXHAUSTED"),
        trigger("leave_elsewhere"),
        [guard_name("review_cycle_repeat_exhausted")],
    )
    .unwrap();
    let refusal = rebuilt(elsewhere, Some(from_elsewhere), exhausted_guard(CYCLE)).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("must reference the transition source state"),
        "{refusal}"
    );
}

#[test]
fn adding_the_guard_kind_keeps_existing_definitions_byte_identical() {
    let unrouted = definition(false);
    let encoded = serde_json::to_string(&unrouted).unwrap();
    assert!(!encoded.contains("state_repeat_exhausted"));
    let decoded: ProtocolDefinition = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.digest().unwrap(), unrouted.digest().unwrap());
}

#[test]
fn a_repeating_state_without_an_exhaustion_exit_is_warned_about_but_publishable() {
    const REASON: &str = "repeating state has no state_repeat_exhausted exit";
    let warned = |definition: &ProtocolDefinition| {
        let report = definition.analyze();
        assert!(report.is_valid(), "{:?}", report.findings());
        report
            .findings()
            .iter()
            .any(|finding| finding.defect().to_string().contains(REASON))
    };
    assert!(warned(&definition(false)));
    assert!(!warned(&definition(true)));
}
