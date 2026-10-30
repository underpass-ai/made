//! A protocol with everything the fold has to reproduce: several
//! states, a repeating step, a retrying step, a human guard that
//! blocks two moves, interventions and two seats.

use made_core::entities::{
    ContextItem, ContextSummary, ExternalContextBundle, ProtocolDefinition, ProtocolEvidencePack,
    ProtocolInstance,
};
use made_core::value_objects::{
    Attributes, DurationMs, GuardCondition, GuardName, IdempotencyKey, LeaseOwnerId,
    ProtocolContext, ProtocolEvidenceSourceId, ProtocolGuard, ProtocolGuardDeferralContent,
    ProtocolId, ProtocolInterventionContent, ProtocolInterventionId, ProtocolName, ProtocolRole,
    ProtocolState, ProtocolStep, ProtocolTransition, ProtocolVersion, RepeatUntilCondition,
    RetryPolicy, RoleAction, RoleId, Specialty, StateId, StepAttempt, StepHandlerConfig,
    StepHandlerKind, StepId, StepIteration, StepLease, StepOutput, StepOutputField,
    StepRepeatPolicy, StepStatus, TransitionTrigger,
};
use serde_json::json;
use std::collections::BTreeMap;
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

pub(crate) const OPENED_AT: OffsetDateTime = datetime!(2026-09-16 09:00:00 UTC);

/// `OPENED_AT` plus `minutes`.
pub(crate) fn at(minutes: i64) -> OffsetDateTime {
    OPENED_AT + Duration::minutes(minutes)
}

pub(crate) fn state(raw: &str) -> StateId {
    StateId::new(raw).unwrap()
}

pub(crate) fn step(raw: &str) -> StepId {
    StepId::new(raw).unwrap()
}

pub(crate) fn trigger(raw: &str) -> TransitionTrigger {
    TransitionTrigger::new(raw).unwrap()
}

pub(crate) fn role(raw: &str) -> RoleId {
    RoleId::new(raw).unwrap()
}

pub(crate) fn guard(raw: &str) -> GuardName {
    GuardName::new(raw).unwrap()
}

pub(crate) fn item(raw: &str) -> ProtocolInterventionId {
    ProtocolInterventionId::new(raw).unwrap()
}

pub(crate) fn specialty(raw: &str) -> Specialty {
    Specialty::new(raw).unwrap()
}

pub(crate) fn content(message: &str) -> ProtocolInterventionContent {
    ProtocolInterventionContent::new(message, Attributes::empty()).unwrap()
}

pub(crate) fn deferral() -> ProtocolGuardDeferralContent {
    ProtocolGuardDeferralContent::new(
        "Not yet.",
        "The plan has not been read.",
        vec!["The plan is read.".to_owned()],
    )
    .unwrap()
}

/// A lease taken at `acquired_at` for five minutes.
pub(crate) fn lease(key: &str, acquired_at: OffsetDateTime) -> StepLease {
    StepLease::new(
        LeaseOwnerId::new("host-1").unwrap(),
        IdempotencyKey::new(key).unwrap(),
        acquired_at,
        acquired_at + Duration::minutes(5),
    )
    .unwrap()
}

pub(crate) fn readiness(ready: bool) -> StepOutput {
    StepOutput::new(Attributes::new(BTreeMap::from([("ready".to_owned(), json!(ready))])).unwrap())
}

pub(crate) fn evidence_pack(source: &str) -> ProtocolEvidencePack {
    let bundle = ExternalContextBundle::new(
        "bundle-1",
        "1.0",
        Some(ContextSummary::new("The wiki fixes the scope.", Attributes::empty()).unwrap()),
        vec![ContextItem::new(
            "scope",
            "page",
            "Scope",
            Some("The scope is fixed.".to_owned()),
            Attributes::empty(),
            Vec::new(),
        )
        .unwrap()],
        Vec::new(),
        Attributes::empty(),
    )
    .unwrap();
    ProtocolEvidencePack::new(
        ProtocolEvidenceSourceId::new(source).unwrap(),
        bundle,
        at(-60),
    )
    .unwrap()
}

/// `drafting` runs `plan` (three attempts, repeated until it says it
/// is ready, twice at most); `submit` moves to `review` once `plan`
/// completed; `review` runs `check` (two attempts); `approve` moves
/// to `done` once `check` completed and a person approved; `abandon`
/// moves straight to `done` on a person's approval alone. The
/// facilitator runs everything and asks; the observer answers.
pub(crate) fn definition() -> ProtocolDefinition {
    let handler = StepHandlerKind::new("multiagent_round").unwrap();
    let plan = ProtocolStep::new(
        step("plan"),
        state("drafting"),
        handler.clone(),
        StepHandlerConfig::empty(),
        RetryPolicy::new(StepAttempt::new(3).unwrap(), DurationMs::ZERO),
        None,
    )
    .with_repeat_policy(StepRepeatPolicy::new(
        RepeatUntilCondition::output_field_equals(
            StepOutputField::new("ready").unwrap(),
            json!(true),
        ),
        StepIteration::new(2).unwrap(),
    ));
    let check = ProtocolStep::new(
        step("check"),
        state("review"),
        handler,
        StepHandlerConfig::empty(),
        RetryPolicy::new(StepAttempt::new(2).unwrap(), DurationMs::ZERO),
        None,
    );
    let plan_done = ProtocolGuard::new(
        guard("plan_done"),
        GuardCondition::StepStatus {
            step_id: step("plan"),
            status: StepStatus::Completed,
        },
    );
    let check_done = ProtocolGuard::new(
        guard("check_done"),
        GuardCondition::StepStatus {
            step_id: step("check"),
            status: StepStatus::Completed,
        },
    );
    let human_approved = ProtocolGuard::new(guard("human_approved"), GuardCondition::HumanApproval);
    let submit = ProtocolTransition::new(
        state("drafting"),
        state("review"),
        trigger("submit"),
        vec![guard("plan_done")],
    )
    .unwrap();
    let approve = ProtocolTransition::new(
        state("review"),
        state("done"),
        trigger("approve"),
        vec![guard("check_done"), guard("human_approved")],
    )
    .unwrap();
    let abandon = ProtocolTransition::new(
        state("drafting"),
        state("done"),
        trigger("abandon"),
        vec![guard("human_approved")],
    )
    .unwrap();
    let facilitator = ProtocolRole::new(
        role("facilitator"),
        vec![
            RoleAction::step(step("plan")),
            RoleAction::step(step("check")),
            RoleAction::transition(trigger("submit")),
            RoleAction::transition(trigger("approve")),
            RoleAction::transition(trigger("abandon")),
            RoleAction::request_intervention(),
        ],
    )
    .unwrap();
    let observer = ProtocolRole::new(
        role("observer"),
        vec![RoleAction::respond_to_intervention()],
    )
    .unwrap();

    ProtocolDefinition::new(
        ProtocolName::new("fold_protocol").unwrap(),
        ProtocolVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            ProtocolState::initial(state("drafting")),
            ProtocolState::intermediate(state("review")),
            ProtocolState::terminal(state("done")),
        ],
        vec![submit, approve, abandon],
        vec![plan, check],
        vec![plan_done, check_done, human_approved],
        vec![facilitator, observer],
    )
    .unwrap()
}

pub(crate) fn opened(definition: &ProtocolDefinition) -> ProtocolInstance {
    ProtocolInstance::start(
        ProtocolId::new("protocol-fold").unwrap(),
        definition,
        ProtocolContext::empty(),
        OPENED_AT,
    )
    .expect("required protocol inputs")
}
