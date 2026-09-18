use made_core::entities::ceremony_commands::ApplyTransition;
use made_core::entities::ceremony_events::{InstanceImported, StepCompleted};
use made_core::entities::{CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyContext, CeremonyGuard, CeremonyId, CeremonyName, CeremonyRevision, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, GuardCondition, GuardName,
    MaxBounces, MaxTransitions, RetryPolicy, RoleAction, RoleId, StateId, StateIteration,
    StepAttempt, StepHandlerConfig, StepHandlerKind, StepId, StepOutput, StepResult, StepStatus,
    TransitionTrigger,
};
use time::OffsetDateTime;

fn state(raw: &str) -> StateId {
    StateId::new(raw).unwrap()
}

fn trigger(raw: &str) -> TransitionTrigger {
    TransitionTrigger::new(raw).unwrap()
}

fn transition(from: &str, to: &str, on: &str, guards: Vec<GuardName>) -> CeremonyTransition {
    CeremonyTransition::new(state(from), state(to), trigger(on), guards).unwrap()
}

fn definition(
    transitions: Vec<CeremonyTransition>,
    guards: Vec<CeremonyGuard>,
    max_transitions: Option<u32>,
    max_bounces: Option<u32>,
) -> Result<CeremonyDefinition, DomainError> {
    let actions = transitions
        .iter()
        .map(|transition| RoleAction::transition(transition.trigger().clone()))
        .collect::<Vec<_>>();
    CeremonyDefinition::new_with_transition_budgets(
        CeremonyName::new("bounded_cycle").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(state("A")),
            CeremonyState::intermediate(state("B")),
            CeremonyState::terminal(state("DONE")),
        ],
        transitions,
        Vec::new(),
        guards,
        vec![CeremonyRole::new(RoleId::new("DRIVER").unwrap(), actions).unwrap()],
        max_transitions.map(|limit| MaxTransitions::new(limit).unwrap()),
        max_bounces.map(|limit| MaxBounces::new(limit).unwrap()),
    )
}

fn start(definition: &CeremonyDefinition) -> (CeremonyInstance, Vec<CeremonyEvent>) {
    let opening = CeremonyInstance::decide_start(
        CeremonyId::new("bounded-cycle-1").unwrap(),
        definition,
        CeremonyContext::empty(),
        None,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    let instance = CeremonyInstance::rehydrate(&opening).unwrap();
    (instance, opening)
}

fn apply(
    instance: &mut CeremonyInstance,
    definition: &CeremonyDefinition,
    on: &str,
    events: &mut Vec<CeremonyEvent>,
) -> Result<(), DomainError> {
    let decided = instance.decide(
        &CeremonyCommand::ApplyTransition(ApplyTransition {
            role_id: Some(RoleId::new("DRIVER").unwrap()),
            trigger: trigger(on),
            now: OffsetDateTime::UNIX_EPOCH,
        }),
        definition,
    )?;
    for event in &decided {
        instance.apply(event);
    }
    events.extend(decided);
    Ok(())
}

#[test]
fn cyclic_definitions_require_either_positive_budget() {
    let self_loop = vec![transition("A", "A", "again", Vec::new())];
    assert!(definition(self_loop.clone(), Vec::new(), None, None).is_err());
    assert!(definition(self_loop.clone(), Vec::new(), Some(1), None).is_ok());
    assert!(definition(self_loop, Vec::new(), None, Some(1)).is_ok());

    let multi_state = vec![
        transition("A", "B", "next", Vec::new()),
        transition("B", "A", "back", Vec::new()),
    ];
    assert!(definition(multi_state.clone(), Vec::new(), None, None).is_err());
    assert!(definition(multi_state, Vec::new(), Some(2), None).is_ok());
}

#[test]
fn exact_edge_and_total_limits_are_derived_from_replayable_history() {
    let bounded = definition(
        vec![
            transition("A", "B", "next", Vec::new()),
            transition("B", "A", "back", Vec::new()),
        ],
        Vec::new(),
        Some(3),
        Some(1),
    )
    .unwrap();
    let (mut instance, mut events) = start(&bounded);

    apply(&mut instance, &bounded, "next", &mut events).unwrap();
    apply(&mut instance, &bounded, "back", &mut events).unwrap();
    let before = instance.clone();
    let error = apply(&mut instance, &bounded, "next", &mut events).unwrap_err();
    assert!(matches!(
        error,
        DomainError::InvariantViolated {
            reason: "ceremony transition bounce limit exhausted"
        }
    ));
    assert_eq!(instance.transitions(), before.transitions());
    assert_eq!(instance.current_state(), before.current_state());

    let mut reopened = CeremonyInstance::rehydrate(&events).unwrap();
    let replay_error = apply(&mut reopened, &bounded, "next", &mut Vec::new()).unwrap_err();
    assert_eq!(replay_error, error);

    let imported = CeremonyEvent::InstanceImported(InstanceImported {
        ceremony_id: reopened.id().clone(),
        definition_name: reopened.definition_name().clone(),
        definition_version: reopened.definition_version().clone(),
        snapshot: Box::new(reopened.clone()),
        legacy_journal_head_hash: None,
        legacy_revision: CeremonyRevision::INITIAL,
        imported_at: OffsetDateTime::UNIX_EPOCH,
    });
    let mut imported = CeremonyInstance::rehydrate([&imported]).unwrap();
    let imported_error = apply(&mut imported, &bounded, "next", &mut Vec::new()).unwrap_err();
    assert_eq!(imported_error, error);

    let total_only = definition(
        vec![
            transition("A", "B", "next", Vec::new()),
            transition("B", "A", "back", Vec::new()),
        ],
        Vec::new(),
        Some(2),
        None,
    )
    .unwrap();
    let (mut total_instance, mut total_events) = start(&total_only);
    apply(&mut total_instance, &total_only, "next", &mut total_events).unwrap();
    apply(&mut total_instance, &total_only, "back", &mut total_events).unwrap();
    assert!(matches!(
        apply(&mut total_instance, &total_only, "next", &mut total_events),
        Err(DomainError::InvariantViolated {
            reason: "ceremony transition limit exhausted"
        })
    ));
}

#[test]
fn an_outgoing_human_guard_controls_the_component_but_an_unrelated_guard_does_not() {
    let human = GuardName::new("human_exit").unwrap();
    let guards = vec![CeremonyGuard::new(
        human.clone(),
        GuardCondition::HumanApproval,
    )];
    let controlled = definition(
        vec![
            transition("A", "A", "again", Vec::new()),
            transition("A", "DONE", "finish", vec![human.clone()]),
        ],
        guards.clone(),
        None,
        Some(1),
    )
    .unwrap();
    assert!(!controlled.analyze().warnings().any(|finding| {
        finding
            .defect()
            .to_string()
            .contains("no human-controlled state")
    }));

    let unrelated = definition(
        vec![transition("A", "A", "again", Vec::new())],
        guards,
        None,
        Some(1),
    )
    .unwrap();
    assert!(unrelated.analyze().warnings().any(|finding| {
        finding
            .defect()
            .to_string()
            .contains("no human-controlled state")
    }));
}

#[test]
fn capped_revisit_preserves_the_existing_completed_step_record() {
    let work = StepId::new("work").unwrap();
    let next = transition("A", "B", "next", Vec::new());
    let back = transition("B", "A", "back", Vec::new());
    let definition = CeremonyDefinition::new_with_transition_budgets(
        CeremonyName::new("bounded_revisit").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(state("A")),
            CeremonyState::intermediate(state("B")),
        ],
        vec![next, back],
        vec![CeremonyStep::new(
            work.clone(),
            state("A"),
            StepHandlerKind::new("noop").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::default(),
            None,
        )],
        Vec::new(),
        vec![CeremonyRole::new(
            RoleId::new("DRIVER").unwrap(),
            vec![
                RoleAction::step(work.clone()),
                RoleAction::transition(trigger("next")),
                RoleAction::transition(trigger("back")),
            ],
        )
        .unwrap()],
        None,
        Some(MaxBounces::new(1).unwrap()),
    )
    .unwrap();
    let (mut instance, mut events) = start(&definition);
    instance.apply(&CeremonyEvent::StepCompleted(StepCompleted {
        step_id: work.clone(),
        state_iteration: Some(StateIteration::FIRST),
        iteration: made_core::value_objects::StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        next_iteration: None,
        finished_by: RoleId::new("DRIVER").unwrap(),
        finished_at: OffsetDateTime::UNIX_EPOCH,
    }));
    assert_eq!(
        instance.step_record(&work).unwrap().status(),
        StepStatus::Completed
    );

    apply(&mut instance, &definition, "next", &mut events).unwrap();
    apply(&mut instance, &definition, "back", &mut events).unwrap();

    let record = instance.step_record(&work).unwrap();
    assert_eq!(record.status(), StepStatus::Completed);
    assert_eq!(record.state_iteration(), StateIteration::FIRST);
    assert!(instance.step_record_history(&work).is_empty());
}
