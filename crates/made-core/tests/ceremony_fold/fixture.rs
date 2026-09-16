//! A ceremony with everything the fold has to reproduce: several
//! states, a repeating step, a retrying step, a human guard that
//! blocks two moves, interventions and two seats.

use made_core::entities::{
    CeremonyDefinition, CeremonyEvidencePack, CeremonyInstance, ContextItem, ContextSummary,
    ExternalContextBundle,
};
use made_core::value_objects::{
    Attributes, CeremonyContext, CeremonyEvidenceSourceId, CeremonyGuard,
    CeremonyGuardDeferralContent, CeremonyId, CeremonyInterventionContent, CeremonyInterventionId,
    CeremonyName, CeremonyRole, CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion,
    DurationMs, GuardCondition, GuardName, IdempotencyKey, LeaseOwnerId, RepeatUntilCondition,
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

pub(crate) fn item(raw: &str) -> CeremonyInterventionId {
    CeremonyInterventionId::new(raw).unwrap()
}

pub(crate) fn specialty(raw: &str) -> Specialty {
    Specialty::new(raw).unwrap()
}

pub(crate) fn content(message: &str) -> CeremonyInterventionContent {
    CeremonyInterventionContent::new(message, Attributes::empty()).unwrap()
}

pub(crate) fn deferral() -> CeremonyGuardDeferralContent {
    CeremonyGuardDeferralContent::new(
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

pub(crate) fn evidence_pack(source: &str) -> CeremonyEvidencePack {
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
    CeremonyEvidencePack::new(
        CeremonyEvidenceSourceId::new(source).unwrap(),
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
pub(crate) fn definition() -> CeremonyDefinition {
    let handler = StepHandlerKind::new("multiagent_round").unwrap();
    let plan = CeremonyStep::new(
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
    let check = CeremonyStep::new(
        step("check"),
        state("review"),
        handler,
        StepHandlerConfig::empty(),
        RetryPolicy::new(StepAttempt::new(2).unwrap(), DurationMs::ZERO),
        None,
    );
    let plan_done = CeremonyGuard::new(
        guard("plan_done"),
        GuardCondition::StepStatus {
            step_id: step("plan"),
            status: StepStatus::Completed,
        },
    );
    let check_done = CeremonyGuard::new(
        guard("check_done"),
        GuardCondition::StepStatus {
            step_id: step("check"),
            status: StepStatus::Completed,
        },
    );
    let human_approved = CeremonyGuard::new(guard("human_approved"), GuardCondition::HumanApproval);
    let submit = CeremonyTransition::new(
        state("drafting"),
        state("review"),
        trigger("submit"),
        vec![guard("plan_done")],
    )
    .unwrap();
    let approve = CeremonyTransition::new(
        state("review"),
        state("done"),
        trigger("approve"),
        vec![guard("check_done"), guard("human_approved")],
    )
    .unwrap();
    let abandon = CeremonyTransition::new(
        state("drafting"),
        state("done"),
        trigger("abandon"),
        vec![guard("human_approved")],
    )
    .unwrap();
    let facilitator = CeremonyRole::new(
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
    let observer = CeremonyRole::new(
        role("observer"),
        vec![RoleAction::respond_to_intervention()],
    )
    .unwrap();

    CeremonyDefinition::new(
        CeremonyName::new("fold_ceremony").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(state("drafting")),
            CeremonyState::intermediate(state("review")),
            CeremonyState::terminal(state("done")),
        ],
        vec![submit, approve, abandon],
        vec![plan, check],
        vec![plan_done, check_done, human_approved],
        vec![facilitator, observer],
    )
    .unwrap()
}

pub(crate) fn opened(definition: &CeremonyDefinition) -> CeremonyInstance {
    CeremonyInstance::start(
        CeremonyId::new("ceremony-fold").unwrap(),
        definition,
        CeremonyContext::empty(),
        OPENED_AT,
    )
}
