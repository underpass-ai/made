use made_core::entities::CeremonyDefinitionDraft;
use made_core::value_objects::{
    CeremonyGuard, CeremonyName, CeremonyRole, CeremonyState, CeremonyStep, CeremonyTransition,
    CeremonyValidationLocus, CeremonyVersion, GuardCondition, GuardName, JoinStepCount,
    MaxTransitions, RetryPolicy, RoleAction, RoleId, StateExecution, StateId, StepHandlerConfig,
    StepHandlerKind, StepId, StepStatus, TransitionTrigger,
};

fn state(name: &str) -> StateId {
    StateId::new(name).unwrap()
}

fn step(name: &str, state_name: &str) -> CeremonyStep {
    CeremonyStep::new(
        StepId::new(name).unwrap(),
        state(state_name),
        StepHandlerKind::new("host_callback").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::default(),
        None,
    )
}

fn edge(from: &str, to: &str, trigger: &str, guarded: bool) -> CeremonyTransition {
    CeremonyTransition::new(
        state(from),
        state(to),
        TransitionTrigger::new(trigger).unwrap(),
        if guarded {
            vec![GuardName::new("join").unwrap()]
        } else {
            vec![]
        },
    )
    .unwrap()
}

fn draft(condition: GuardCondition, edges: Vec<CeremonyTransition>) -> CeremonyDefinitionDraft {
    let steps = vec![
        step("a", "REVIEW"),
        step("b", "REVIEW"),
        step("c", "SYNTHESIS"),
    ];
    let mut roles = steps
        .iter()
        .map(|step| {
            CeremonyRole::new(
                RoleId::new(step.id().as_str()).unwrap(),
                vec![RoleAction::step(step.id().clone())],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    roles.push(
        CeremonyRole::new(
            RoleId::new("driver").unwrap(),
            edges
                .iter()
                .map(|e| RoleAction::transition(e.trigger().clone())),
        )
        .unwrap(),
    );
    CeremonyDefinitionDraft::new(
        CeremonyName::new("global_completion").unwrap(),
        CeremonyVersion::v1(),
        None,
        vec![],
        vec![],
        vec![
            CeremonyState::initial(state("REVIEW")).with_execution(StateExecution::Concurrent),
            CeremonyState::intermediate(state("SYNTHESIS")),
            CeremonyState::terminal(state("DONE")),
        ],
        edges,
        steps,
        vec![CeremonyGuard::new(
            GuardName::new("join").unwrap(),
            condition,
        )],
        roles,
    )
    .with_max_transitions(MaxTransitions::new(6).unwrap())
}

fn linear_edges() -> Vec<CeremonyTransition> {
    vec![
        edge("REVIEW", "SYNTHESIS", "reviewed", true),
        edge("SYNTHESIS", "DONE", "finish", true),
    ]
}

fn global_warnings(draft: &CeremonyDefinitionDraft) -> usize {
    draft
        .analyze()
        .warnings()
        .filter(|finding| {
            finding
                .defect()
                .to_string()
                .contains("all_steps_completed is global")
        })
        .count()
}

#[test]
fn warns_at_review_to_synthesis_without_blocking_publication() {
    let draft = draft(GuardCondition::AllStepsCompleted, linear_edges());
    let analysis = draft.analyze();
    assert!(analysis.is_valid());
    let warning = analysis
        .warnings()
        .find(|finding| {
            finding
                .defect()
                .to_string()
                .contains("all_steps_completed is global")
        })
        .unwrap();
    assert_eq!(
        warning.locus(),
        &CeremonyValidationLocus::transition(
            state("REVIEW"),
            TransitionTrigger::new("reviewed").unwrap()
        )
    );
    assert_eq!(global_warnings(&draft), 1);
    let published = draft.publish().unwrap();
    assert_eq!(published.analyze(), analysis);
}

#[test]
fn counted_and_explicit_source_joins_do_not_get_global_warning() {
    for condition in [
        GuardCondition::StepsCompleted(JoinStepCount::new(2).unwrap()),
        GuardCondition::StepStatus {
            step_id: StepId::new("a").unwrap(),
            status: StepStatus::Completed,
        },
    ] {
        assert_eq!(global_warnings(&draft(condition, linear_edges())), 0);
    }
}

#[test]
fn global_guard_on_return_after_earlier_execution_is_not_warned() {
    let draft = draft(
        GuardCondition::AllStepsCompleted,
        vec![
            edge("REVIEW", "SYNTHESIS", "out", false),
            edge("SYNTHESIS", "REVIEW", "back", true),
            edge("REVIEW", "DONE", "finish", true),
        ],
    );
    assert_eq!(global_warnings(&draft), 0);
    assert!(draft.publish().is_ok());
}

#[test]
fn a_cycle_does_not_hide_a_blocked_first_visit() {
    let draft = draft(
        GuardCondition::AllStepsCompleted,
        vec![
            edge("REVIEW", "SYNTHESIS", "out", true),
            edge("SYNTHESIS", "REVIEW", "back", false),
            edge("SYNTHESIS", "DONE", "finish", false),
        ],
    );
    assert_eq!(global_warnings(&draft), 1);
    assert!(draft.publish().is_ok());
}

#[test]
fn invalid_graph_is_reported_without_speculative_guard_warning() {
    let draft = draft(
        GuardCondition::AllStepsCompleted,
        vec![edge("REVIEW", "MISSING", "out", true)],
    );
    assert!(!draft.analyze().is_valid());
    assert_eq!(global_warnings(&draft), 0);
}
