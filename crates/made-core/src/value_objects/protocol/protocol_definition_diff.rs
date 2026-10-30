use crate::entities::ProtocolDefinition;

use super::{
    ContextWrites, DynamicRoleBinding, InputRequirement, MaxBounces, MaxTransitions,
    ProtocolChangeImpact, ProtocolChangeKind, ProtocolDefinitionChange, ProtocolInputDefinition,
    ProtocolValidationLocus, StateId, TransitionTrigger,
};

fn is_required(input: &ProtocolInputDefinition) -> bool {
    matches!(input.requirement(), InputRequirement::Required)
}

/// Everything that differs between two definitions of one protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolDefinitionDiff {
    changes: Vec<ProtocolDefinitionChange>,
}

impl ProtocolDefinitionDiff {
    /// Compare two definitions, earlier first.
    ///
    /// The two need not be versions of the same protocol: comparing
    /// unrelated documents is a strange thing to ask for but not a
    /// wrong one, and refusing it would mean deciding on the caller's
    /// behalf what counts as related.
    #[must_use]
    pub fn between(before: &ProtocolDefinition, after: &ProtocolDefinition) -> Self {
        let mut changes = Vec::new();
        diff_states(before, after, &mut changes);
        diff_transitions(before, after, &mut changes);
        diff_steps(before, after, &mut changes);
        diff_guards(before, after, &mut changes);
        diff_roles(before, after, &mut changes);
        diff_max_parallel(before, after, &mut changes);
        diff_transition_budgets(before, after, &mut changes);
        diff_shape(before, after, &mut changes);
        Self { changes }
    }

    #[must_use]
    pub fn changes(&self) -> &[ProtocolDefinitionChange] {
        &self.changes
    }

    #[must_use]
    pub fn is_identical(&self) -> bool {
        self.changes.is_empty()
    }

    /// Whether any single change could strand a running session. One is
    /// enough: a version that can strand a session is not one to adopt
    /// mid-flight, however much else about it is harmless.
    #[must_use]
    pub fn strands_running_sessions(&self) -> bool {
        self.changes.iter().any(|change| change.impact().strands())
    }

    #[must_use]
    pub fn strand_count(&self) -> usize {
        self.changes
            .iter()
            .filter(|change| change.impact().strands())
            .count()
    }
}

fn diff_transition_budgets(
    before: &ProtocolDefinition,
    after: &ProtocolDefinition,
    changes: &mut Vec<ProtocolDefinitionChange>,
) {
    diff_transition_budget(
        before.max_transitions().map(MaxTransitions::get),
        after.max_transitions().map(MaxTransitions::get),
        changes,
        "the total number of transitions a running session may apply",
    );
    diff_transition_budget(
        before.max_bounces().map(MaxBounces::get),
        after.max_bounces().map(MaxBounces::get),
        changes,
        "the number of times a running session may apply one exact edge",
    );
}

fn diff_transition_budget(
    before: Option<u32>,
    after: Option<u32>,
    changes: &mut Vec<ProtocolDefinitionChange>,
    detail: &'static str,
) {
    if before == after {
        return;
    }
    let impact = match (before, after) {
        (None, Some(_)) => ProtocolChangeImpact::Strands,
        (Some(old), Some(new)) if new < old => ProtocolChangeImpact::Strands,
        _ => ProtocolChangeImpact::Carries,
    };
    record(
        changes,
        ProtocolChangeKind::Altered,
        ProtocolValidationLocus::Definition,
        impact,
        detail,
    );
}

fn record(
    changes: &mut Vec<ProtocolDefinitionChange>,
    kind: ProtocolChangeKind,
    locus: ProtocolValidationLocus,
    impact: ProtocolChangeImpact,
    detail: &'static str,
) {
    changes.push(ProtocolDefinitionChange::new(kind, locus, impact, detail));
}

fn diff_max_parallel(
    before: &ProtocolDefinition,
    after: &ProtocolDefinition,
    changes: &mut Vec<ProtocolDefinitionChange>,
) {
    if before.max_parallel() == after.max_parallel() {
        return;
    }
    let impact = if after.max_parallel() < before.max_parallel() {
        ProtocolChangeImpact::Strands
    } else {
        ProtocolChangeImpact::Carries
    };
    record(
        changes,
        ProtocolChangeKind::Altered,
        ProtocolValidationLocus::Definition,
        impact,
        "how many steps may be claimed concurrently",
    );
}

fn diff_states(
    before: &ProtocolDefinition,
    after: &ProtocolDefinition,
    changes: &mut Vec<ProtocolDefinitionChange>,
) {
    for (id, state) in before.states() {
        let Some(now) = after.states().get(id) else {
            // A session sitting in a state that no longer exists has nowhere
            // to be.
            record(
                changes,
                ProtocolChangeKind::Removed,
                ProtocolValidationLocus::state(id.clone()),
                ProtocolChangeImpact::Strands,
                "a session in this state would have nowhere to be",
            );
            continue;
        };
        let locus = ProtocolValidationLocus::state(id.clone());
        if now.kind() != state.kind() {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus.clone(),
                // Becoming terminal ends sessions early; ceasing to be
                // terminal leaves finished ones unfinished.
                ProtocolChangeImpact::Strands,
                "whether the session may start or finish here",
            );
        }
        if now.execution() != state.execution() {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus.clone(),
                ProtocolChangeImpact::Strands,
                "whether work in this state runs sequentially or concurrently",
            );
        }
        if now.repeat_policy() != state.repeat_policy() {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus,
                ProtocolChangeImpact::Strands,
                "when and how often this state repeats",
            );
        }
    }
    for id in after.states().keys() {
        if !before.states().contains_key(id) {
            record(
                changes,
                ProtocolChangeKind::Added,
                ProtocolValidationLocus::state(id.clone()),
                ProtocolChangeImpact::Carries,
                "a state no running session is in yet",
            );
        }
    }
}

fn transition_key(from: &StateId, trigger: &TransitionTrigger) -> (String, String) {
    (from.as_str().to_owned(), trigger.as_str().to_owned())
}

fn diff_transitions(
    before: &ProtocolDefinition,
    after: &ProtocolDefinition,
    changes: &mut Vec<ProtocolDefinitionChange>,
) {
    for old in before.transitions() {
        let key = transition_key(old.from(), old.trigger());
        let now = after
            .transitions()
            .iter()
            .find(|candidate| transition_key(candidate.from(), candidate.trigger()) == key);
        let locus = ProtocolValidationLocus::transition(old.from().clone(), old.trigger().clone());
        match now {
            // A session may have been counting on exactly this move.
            None => record(
                changes,
                ProtocolChangeKind::Removed,
                locus,
                ProtocolChangeImpact::Strands,
                "a way out of the state it leaves",
            ),
            Some(now) if now.to() != old.to() => record(
                changes,
                ProtocolChangeKind::Altered,
                locus,
                ProtocolChangeImpact::Strands,
                "where it leads",
            ),
            Some(now) if now.required_guards() != old.required_guards() => {
                // Guards added can block a session that was about to
                // move; guards dropped only ever let it through.
                let impact = if now.required_guards().is_superset(old.required_guards()) {
                    ProtocolChangeImpact::Strands
                } else {
                    ProtocolChangeImpact::Carries
                };
                record(
                    changes,
                    ProtocolChangeKind::Altered,
                    locus,
                    impact,
                    "what has to hold before it can fire",
                );
            }
            Some(_) => {}
        }
    }
    for now in after.transitions() {
        let key = transition_key(now.from(), now.trigger());
        if !before
            .transitions()
            .iter()
            .any(|candidate| transition_key(candidate.from(), candidate.trigger()) == key)
        {
            record(
                changes,
                ProtocolChangeKind::Added,
                ProtocolValidationLocus::transition(now.from().clone(), now.trigger().clone()),
                ProtocolChangeImpact::Carries,
                "another way out of a state",
            );
        }
    }
}

fn diff_steps(
    before: &ProtocolDefinition,
    after: &ProtocolDefinition,
    changes: &mut Vec<ProtocolDefinitionChange>,
) {
    for (id, old) in before.steps() {
        let locus = ProtocolValidationLocus::step(id.clone());
        let Some(now) = after.steps().get(id) else {
            // A guard that waits on this step will wait forever, and a
            // session that has not run it never will.
            record(
                changes,
                ProtocolChangeKind::Removed,
                locus,
                ProtocolChangeImpact::Strands,
                "work a session may not have done yet",
            );
            continue;
        };
        if now.state_id() != old.state_id() {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus.clone(),
                ProtocolChangeImpact::Strands,
                "the state it belongs to",
            );
        }
        if now.handler_kind() != old.handler_kind() {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus.clone(),
                ProtocolChangeImpact::Carries,
                "who does the work",
            );
        }
        if now.handler_config() != old.handler_config() {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus.clone(),
                ProtocolChangeImpact::Carries,
                "how the work is asked for",
            );
        }
        if now.dynamic_role_binding() != old.dynamic_role_binding() {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus.clone(),
                dynamic_role_binding_impact(old.dynamic_role_binding(), now.dynamic_role_binding()),
                "how the role that may claim the step is resolved",
            );
        }
        if now.context_writes() != old.context_writes() {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus.clone(),
                context_writes_impact(old.context_writes(), now.context_writes()),
                "which successful output fields update protocol context",
            );
        }
        if now.aggregation() != old.aggregation() {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus.clone(),
                ProtocolChangeImpact::Carries,
                "how predecessor sibling outputs are aggregated",
            );
        }
        if now.retry_policy() != old.retry_policy()
            || now.timeout() != old.timeout()
            || now.repeat_policy() != old.repeat_policy()
        {
            record(
                changes,
                ProtocolChangeKind::Altered,
                locus,
                ProtocolChangeImpact::Carries,
                "how long it may take, how often it may be retried, or when repetition stops",
            );
        }
    }
    for id in after.steps().keys() {
        if !before.steps().contains_key(id) {
            // Added work is not harmless: a session already past this
            // step's state will never run it, and a guard waiting on
            // it would never be satisfied.
            record(
                changes,
                ProtocolChangeKind::Added,
                ProtocolValidationLocus::step(id.clone()),
                ProtocolChangeImpact::Strands,
                "work a session may already have moved past",
            );
        }
    }
}

fn dynamic_role_binding_impact(
    before: Option<&DynamicRoleBinding>,
    after: Option<&DynamicRoleBinding>,
) -> ProtocolChangeImpact {
    match (before, after) {
        // Without a selector the statically assigned role remains available.
        (_, None) => ProtocolChangeImpact::Carries,
        (Some(before), Some(after))
            if before.context_key() == after.context_key()
                && before.allowed_roles().is_subset(after.allowed_roles()) =>
        {
            ProtocolChangeImpact::Carries
        }
        // Adding or changing a selector can require unavailable context;
        // removing an allowed role can take away the only claim path.
        (_, Some(_)) => ProtocolChangeImpact::Strands,
    }
}

fn context_writes_impact(_before: &ContextWrites, _after: &ContextWrites) -> ProtocolChangeImpact {
    // Every changed mapping can strand a running session. Adding a write
    // makes its source field mandatory for successful completion; removing or
    // rebinding one changes the context contract later work can depend on.
    ProtocolChangeImpact::Strands
}

fn diff_guards(
    before: &ProtocolDefinition,
    after: &ProtocolDefinition,
    changes: &mut Vec<ProtocolDefinitionChange>,
) {
    for (name, old) in before.guards() {
        let locus = ProtocolValidationLocus::guard(name.clone());
        match after.guards().get(name) {
            None => record(
                changes,
                ProtocolChangeKind::Removed,
                locus,
                ProtocolChangeImpact::Carries,
                "a condition no longer asked for",
            ),
            // An automated guard turned human now waits on a person who
            // was never asked; a human guard turned automated discards
            // an approval already given.
            Some(now) if now.condition() != old.condition() => record(
                changes,
                ProtocolChangeKind::Altered,
                locus,
                ProtocolChangeImpact::Strands,
                "what satisfies it",
            ),
            Some(_) => {}
        }
    }
    for name in after.guards().keys() {
        if !before.guards().contains_key(name) {
            record(
                changes,
                ProtocolChangeKind::Added,
                ProtocolValidationLocus::guard(name.clone()),
                ProtocolChangeImpact::Carries,
                "a condition that only matters where a transition asks for it",
            );
        }
    }
}

fn diff_roles(
    before: &ProtocolDefinition,
    after: &ProtocolDefinition,
    changes: &mut Vec<ProtocolDefinitionChange>,
) {
    for (id, old) in before.roles() {
        let locus = ProtocolValidationLocus::role(id.clone());
        match after.roles().get(id) {
            None => record(
                changes,
                ProtocolChangeKind::Removed,
                locus,
                ProtocolChangeImpact::Strands,
                "whoever was acting as this role can no longer act",
            ),
            Some(now) if now.allowed_actions() != old.allowed_actions() => {
                // Only narrowing takes something away.
                let impact = if old.allowed_actions().is_subset(now.allowed_actions()) {
                    ProtocolChangeImpact::Carries
                } else {
                    ProtocolChangeImpact::Strands
                };
                record(
                    changes,
                    ProtocolChangeKind::Altered,
                    locus,
                    impact,
                    "what this role is allowed to do",
                );
            }
            Some(_) => {}
        }
    }
    for id in after.roles().keys() {
        if !before.roles().contains_key(id) {
            record(
                changes,
                ProtocolChangeKind::Added,
                ProtocolValidationLocus::role(id.clone()),
                ProtocolChangeImpact::Carries,
                "another role at the table",
            );
        }
    }
}

/// Everything that is not the graph: what the protocol says it needs
/// and what it says it produces.
fn diff_shape(
    before: &ProtocolDefinition,
    after: &ProtocolDefinition,
    changes: &mut Vec<ProtocolDefinitionChange>,
) {
    for name in before.inputs().keys() {
        if !after.inputs().contains_key(name) {
            record(
                changes,
                ProtocolChangeKind::Removed,
                ProtocolValidationLocus::input(name.clone()),
                ProtocolChangeImpact::Carries,
                "an input no longer asked for",
            );
        }
    }
    for (name, now) in after.inputs() {
        match before.inputs().get(name) {
            None => record(
                changes,
                ProtocolChangeKind::Added,
                ProtocolValidationLocus::input(name.clone()),
                // A session started without it cannot go back and
                // supply it.
                if is_required(now) {
                    ProtocolChangeImpact::Strands
                } else {
                    ProtocolChangeImpact::Carries
                },
                "an input the protocol now asks for",
            ),
            Some(was) if is_required(was) != is_required(now) => record(
                changes,
                ProtocolChangeKind::Altered,
                ProtocolValidationLocus::input(name.clone()),
                if is_required(now) {
                    ProtocolChangeImpact::Strands
                } else {
                    ProtocolChangeImpact::Carries
                },
                "whether it has to be supplied",
            ),
            Some(_) => {}
        }
    }
    for name in before.outputs().keys() {
        if !after.outputs().contains_key(name) {
            record(
                changes,
                ProtocolChangeKind::Removed,
                ProtocolValidationLocus::output(name.clone()),
                ProtocolChangeImpact::Carries,
                "an output no longer produced",
            );
        }
    }
    for name in after.outputs().keys() {
        if !before.outputs().contains_key(name) {
            record(
                changes,
                ProtocolChangeKind::Added,
                ProtocolValidationLocus::output(name.clone()),
                ProtocolChangeImpact::Carries,
                "an output not produced before",
            );
        }
    }
    if before.description() != after.description() {
        record(
            changes,
            ProtocolChangeKind::Altered,
            ProtocolValidationLocus::Definition,
            ProtocolChangeImpact::Carries,
            "what the protocol says it is for",
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::value_objects::{
        ContextKey, GuardCondition, GuardName, MaxBounces, MaxParallel, MaxTransitions,
        ProtocolGuard, ProtocolName, ProtocolRole, ProtocolState, ProtocolStep, ProtocolTransition,
        ProtocolVersion, RepeatUntilCondition, RetryPolicy, RoleAction, RoleId, StateExecution,
        StateIteration, StateRepeatPolicy, StateRepeatUntilCondition, StepHandlerConfig,
        StepHandlerKind, StepId, StepIteration, StepOutputField, StepRepeatPolicy,
    };

    fn state(id: &str) -> StateId {
        StateId::new(id).unwrap()
    }

    fn trigger(name: &str) -> TransitionTrigger {
        TransitionTrigger::new(name).unwrap()
    }

    fn guard_name(name: &str) -> GuardName {
        GuardName::new(name).unwrap()
    }

    fn step_id(id: &str) -> StepId {
        StepId::new(id).unwrap()
    }

    fn step(id: &str, in_state: &str, handler: &str) -> ProtocolStep {
        ProtocolStep::new(
            step_id(id),
            state(in_state),
            StepHandlerKind::new(handler).unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::default(),
            None,
        )
    }

    fn transition(from: &str, to: &str, on: &str, guards: Vec<GuardName>) -> ProtocolTransition {
        ProtocolTransition::new(state(from), state(to), trigger(on), guards).unwrap()
    }

    fn role(id: &str, actions: BTreeSet<RoleAction>) -> ProtocolRole {
        ProtocolRole::new(RoleId::new(id).unwrap(), actions).unwrap()
    }

    fn facilitator_actions() -> BTreeSet<RoleAction> {
        BTreeSet::from([
            RoleAction::step(step_id("work")),
            RoleAction::transition(trigger("finish")),
        ])
    }

    /// One state to work in, one to finish in, one step, one guard on
    /// the way out, one role allowed to do both.
    struct Draft {
        states: Vec<ProtocolState>,
        transitions: Vec<ProtocolTransition>,
        steps: Vec<ProtocolStep>,
        guards: Vec<ProtocolGuard>,
        roles: Vec<ProtocolRole>,
    }

    impl Draft {
        fn baseline() -> Self {
            Self {
                states: vec![
                    ProtocolState::initial(state("OPEN")),
                    ProtocolState::terminal(state("DONE")),
                ],
                transitions: vec![transition(
                    "OPEN",
                    "DONE",
                    "finish",
                    vec![guard_name("work_done")],
                )],
                steps: vec![step("work", "OPEN", "noop")],
                guards: vec![ProtocolGuard::new(
                    guard_name("work_done"),
                    GuardCondition::AllStepsCompleted,
                )],
                roles: vec![role("FACILITATOR", facilitator_actions())],
            }
        }

        fn build(self) -> ProtocolDefinition {
            self.build_with_budgets(None, None)
        }

        fn build_with_budgets(
            self,
            max_transitions: Option<MaxTransitions>,
            max_bounces: Option<MaxBounces>,
        ) -> ProtocolDefinition {
            ProtocolDefinition::new_with_transition_budgets(
                ProtocolName::new("diffed_protocol").unwrap(),
                ProtocolVersion::v1(),
                None,
                Vec::new(),
                Vec::new(),
                self.states,
                self.transitions,
                self.steps,
                self.guards,
                self.roles,
                max_transitions,
                max_bounces,
            )
            .unwrap()
        }
    }

    fn baseline() -> ProtocolDefinition {
        Draft::baseline().build()
    }

    fn state_repeat(max_iterations: u32) -> StateRepeatPolicy {
        StateRepeatPolicy::new(
            StateIteration::new(max_iterations).unwrap(),
            StateRepeatUntilCondition::new(
                step_id("work"),
                StepOutputField::new("ready").unwrap(),
                serde_json::json!(true),
            ),
        )
    }

    fn assert_single_change(
        diff: &ProtocolDefinitionDiff,
        locus: &ProtocolValidationLocus,
        impact: ProtocolChangeImpact,
        detail: &'static str,
    ) {
        assert_eq!(diff.changes().len(), 1, "{:?}", diff.changes());
        let change = &diff.changes()[0];
        assert_eq!(change.kind(), ProtocolChangeKind::Altered);
        assert_eq!(change.locus(), locus);
        assert_eq!(change.impact(), impact);
        assert_eq!(change.detail(), detail);
    }

    fn dynamic_binding_for(key: &str, roles: &[&str]) -> DynamicRoleBinding {
        DynamicRoleBinding::new(
            ContextKey::new(key).unwrap(),
            roles.iter().map(|role_id| RoleId::new(*role_id).unwrap()),
        )
        .unwrap()
    }

    fn dynamic_binding(key: &str) -> DynamicRoleBinding {
        dynamic_binding_for(key, &["FACILITATOR"])
    }

    fn definition_with_dynamic_roles(allowed_roles: &[&str]) -> ProtocolDefinition {
        let mut draft = Draft::baseline();
        draft.roles.push(role(
            "REVIEWER",
            BTreeSet::from([RoleAction::step(step_id("work"))]),
        ));
        draft.steps = vec![step("work", "OPEN", "noop")
            .with_dynamic_role_binding(dynamic_binding_for("next_role", allowed_roles))];
        draft.build()
    }

    fn context_writes(destination: &str, source: &str) -> ContextWrites {
        ContextWrites::new(BTreeMap::from([(
            ContextKey::new(destination).unwrap(),
            StepOutputField::new(source).unwrap(),
        )]))
    }

    fn assert_step_policy_change(
        diff: &ProtocolDefinitionDiff,
        impact: ProtocolChangeImpact,
        detail: &'static str,
    ) {
        assert_eq!(diff.changes().len(), 1, "{:?}", diff.changes());
        let change = &diff.changes()[0];
        assert_eq!(change.kind(), ProtocolChangeKind::Altered);
        assert_eq!(
            change.locus(),
            &ProtocolValidationLocus::step(step_id("work"))
        );
        assert_eq!(change.impact(), impact);
        assert_eq!(change.detail(), detail);
    }

    #[test]
    fn a_definition_does_not_differ_from_itself() {
        let diff = ProtocolDefinitionDiff::between(&baseline(), &baseline());

        assert!(diff.is_identical());
        assert!(!diff.strands_running_sessions());
    }

    #[test]
    fn changing_state_execution_is_a_stranding_state_change() {
        let mut concurrent = Draft::baseline();
        concurrent.states[0] =
            ProtocolState::initial(state("OPEN")).with_execution(StateExecution::Concurrent);

        assert_single_change(
            &ProtocolDefinitionDiff::between(&baseline(), &concurrent.build()),
            &ProtocolValidationLocus::state(state("OPEN")),
            ProtocolChangeImpact::Strands,
            "whether work in this state runs sequentially or concurrently",
        );
    }

    #[test]
    fn adding_changing_or_removing_state_repeat_is_stranding() {
        let mut repeating_twice = Draft::baseline();
        repeating_twice.states[0] =
            ProtocolState::initial(state("OPEN")).with_repeat_policy(state_repeat(2));
        let repeating_twice = repeating_twice.build();
        let mut repeating_thrice = Draft::baseline();
        repeating_thrice.states[0] =
            ProtocolState::initial(state("OPEN")).with_repeat_policy(state_repeat(3));
        let repeating_thrice = repeating_thrice.build();

        for diff in [
            ProtocolDefinitionDiff::between(&baseline(), &repeating_twice),
            ProtocolDefinitionDiff::between(&repeating_twice, &repeating_thrice),
            ProtocolDefinitionDiff::between(&repeating_thrice, &baseline()),
        ] {
            assert_single_change(
                &diff,
                &ProtocolValidationLocus::state(state("OPEN")),
                ProtocolChangeImpact::Strands,
                "when and how often this state repeats",
            );
        }
    }

    #[test]
    fn lowering_parallel_capacity_strands_and_raising_it_carries() {
        let baseline = baseline();
        let lower = baseline
            .clone()
            .with_max_parallel(MaxParallel::new(2).unwrap());
        let higher = baseline
            .clone()
            .with_max_parallel(MaxParallel::new(4).unwrap());

        assert_single_change(
            &ProtocolDefinitionDiff::between(&baseline, &lower),
            &ProtocolValidationLocus::Definition,
            ProtocolChangeImpact::Strands,
            "how many steps may be claimed concurrently",
        );
        assert_single_change(
            &ProtocolDefinitionDiff::between(&baseline, &higher),
            &ProtocolValidationLocus::Definition,
            ProtocolChangeImpact::Carries,
            "how many steps may be claimed concurrently",
        );
    }

    #[test]
    fn adding_or_lowering_a_transition_budget_can_strand_a_running_session() {
        let baseline = baseline();
        let capped = Draft::baseline().build_with_budgets(
            Some(MaxTransitions::new(10).unwrap()),
            Some(MaxBounces::new(4).unwrap()),
        );
        let added = ProtocolDefinitionDiff::between(&baseline, &capped);
        assert_eq!(added.strand_count(), 2);
        assert!(added.changes().iter().all(|change| {
            change.kind() == ProtocolChangeKind::Altered
                && change.locus() == &ProtocolValidationLocus::Definition
                && change.impact() == ProtocolChangeImpact::Strands
        }));

        let lowered = Draft::baseline().build_with_budgets(
            Some(MaxTransitions::new(5).unwrap()),
            Some(MaxBounces::new(2).unwrap()),
        );
        let tightening = ProtocolDefinitionDiff::between(&capped, &lowered);
        assert_eq!(tightening.strand_count(), 2);
        assert!(tightening.changes().iter().all(|change| {
            change.kind() == ProtocolChangeKind::Altered
                && change.locus() == &ProtocolValidationLocus::Definition
                && change.impact() == ProtocolChangeImpact::Strands
        }));

        let raising = ProtocolDefinitionDiff::between(&lowered, &capped);
        assert!(!raising.strands_running_sessions());
        assert!(raising.changes().iter().all(|change| {
            change.kind() == ProtocolChangeKind::Altered
                && change.locus() == &ProtocolValidationLocus::Definition
                && change.impact() == ProtocolChangeImpact::Carries
        }));

        let removing = ProtocolDefinitionDiff::between(&capped, &baseline);
        assert!(!removing.strands_running_sessions());
        assert!(removing.changes().iter().all(|change| {
            change.kind() == ProtocolChangeKind::Altered
                && change.locus() == &ProtocolValidationLocus::Definition
                && change.impact() == ProtocolChangeImpact::Carries
        }));
    }

    #[test]
    fn removing_a_state_strands_whoever_is_in_it() {
        let mut draft = Draft::baseline();
        draft.states = vec![
            ProtocolState::initial(state("OPEN")),
            ProtocolState::terminal(state("ELSEWHERE")),
        ];
        draft.transitions = vec![transition(
            "OPEN",
            "ELSEWHERE",
            "finish",
            vec![guard_name("work_done")],
        )];

        let diff = ProtocolDefinitionDiff::between(&baseline(), &draft.build());

        assert!(diff.strands_running_sessions());
        let removed = diff
            .changes()
            .iter()
            .find(|change| {
                change.kind() == ProtocolChangeKind::Removed
                    && change.locus() == &ProtocolValidationLocus::state(state("DONE"))
            })
            .expect("the state that went away should be reported");
        assert!(removed.impact().strands());
    }

    #[test]
    fn tightening_a_transition_can_block_a_session_and_relaxing_one_cannot() {
        let mut stricter = Draft::baseline();
        stricter.guards.push(ProtocolGuard::new(
            guard_name("human_approved"),
            GuardCondition::HumanApproval,
        ));
        stricter.transitions = vec![transition(
            "OPEN",
            "DONE",
            "finish",
            vec![guard_name("work_done"), guard_name("human_approved")],
        )];
        assert!(
            ProtocolDefinitionDiff::between(&baseline(), &stricter.build())
                .strands_running_sessions(),
            "a session about to move can be blocked by a guard that was not there"
        );

        let mut looser = Draft::baseline();
        looser.transitions = vec![transition("OPEN", "DONE", "finish", Vec::new())];
        looser.guards = Vec::new();
        assert!(
            !ProtocolDefinitionDiff::between(&baseline(), &looser.build())
                .strands_running_sessions(),
            "dropping a condition only ever lets a session through"
        );
    }

    #[test]
    fn narrowing_a_role_takes_something_away_and_widening_it_does_not() {
        let mut narrowed = Draft::baseline();
        narrowed.roles = vec![role(
            "FACILITATOR",
            BTreeSet::from([RoleAction::step(step_id("work"))]),
        )];
        assert!(
            ProtocolDefinitionDiff::between(&baseline(), &narrowed.build())
                .strands_running_sessions()
        );

        let mut widened = Draft::baseline();
        let mut actions = facilitator_actions();
        actions.insert(RoleAction::request_intervention());
        widened.roles = vec![role("FACILITATOR", actions)];
        assert!(
            !ProtocolDefinitionDiff::between(&baseline(), &widened.build())
                .strands_running_sessions()
        );
    }

    #[test]
    fn changing_how_the_work_is_done_leaves_a_session_where_it_was() {
        let mut draft = Draft::baseline();
        draft.steps = vec![step("work", "OPEN", "deliberation")];

        let diff = ProtocolDefinitionDiff::between(&baseline(), &draft.build());

        assert!(!diff.is_identical());
        assert!(
            !diff.strands_running_sessions(),
            "who does the work is not where the session is"
        );
        assert_eq!(diff.changes().len(), 1);
        assert_eq!(diff.changes()[0].detail(), "who does the work");
    }

    #[test]
    fn changing_repeat_policy_is_a_material_step_change() {
        let mut draft = Draft::baseline();
        draft.steps = vec![
            step("work", "OPEN", "noop").with_repeat_policy(StepRepeatPolicy::new(
                RepeatUntilCondition::output_field_equals(
                    StepOutputField::new("ready").unwrap(),
                    serde_json::json!(true),
                ),
                StepIteration::new(3).unwrap(),
            )),
        ];

        let diff = ProtocolDefinitionDiff::between(&baseline(), &draft.build());

        assert_eq!(diff.changes().len(), 1);
        assert_eq!(
            diff.changes()[0].detail(),
            "how long it may take, how often it may be retried, or when repetition stops"
        );
    }

    #[test]
    fn adding_changing_and_removing_dynamic_role_binding_are_material_step_changes() {
        let mut bound = Draft::baseline();
        bound.steps =
            vec![step("work", "OPEN", "noop")
                .with_dynamic_role_binding(dynamic_binding("next_role"))];
        let bound = bound.build();

        assert_step_policy_change(
            &ProtocolDefinitionDiff::between(&baseline(), &bound),
            ProtocolChangeImpact::Strands,
            "how the role that may claim the step is resolved",
        );

        let mut changed = Draft::baseline();
        changed.steps = vec![step("work", "OPEN", "noop")
            .with_dynamic_role_binding(dynamic_binding("fallback_role"))];
        assert_step_policy_change(
            &ProtocolDefinitionDiff::between(&bound, &changed.build()),
            ProtocolChangeImpact::Strands,
            "how the role that may claim the step is resolved",
        );

        assert_step_policy_change(
            &ProtocolDefinitionDiff::between(&bound, &baseline()),
            ProtocolChangeImpact::Carries,
            "how the role that may claim the step is resolved",
        );
    }

    #[test]
    fn widening_a_dynamic_role_allow_list_carries_and_narrowing_it_strands() {
        let narrow = definition_with_dynamic_roles(&["FACILITATOR"]);
        let wide = definition_with_dynamic_roles(&["FACILITATOR", "REVIEWER"]);

        assert_step_policy_change(
            &ProtocolDefinitionDiff::between(&narrow, &wide),
            ProtocolChangeImpact::Carries,
            "how the role that may claim the step is resolved",
        );
        assert_step_policy_change(
            &ProtocolDefinitionDiff::between(&wide, &narrow),
            ProtocolChangeImpact::Strands,
            "how the role that may claim the step is resolved",
        );
    }

    #[test]
    fn adding_changing_and_removing_context_writes_are_material_step_changes() {
        let mut writing = Draft::baseline();
        writing.steps = vec![step("work", "OPEN", "noop")
            .with_context_writes(context_writes("next_role", "reviewer"))];
        let writing = writing.build();

        assert_step_policy_change(
            &ProtocolDefinitionDiff::between(&baseline(), &writing),
            ProtocolChangeImpact::Strands,
            "which successful output fields update protocol context",
        );

        let mut changed = Draft::baseline();
        changed.steps =
            vec![step("work", "OPEN", "noop")
                .with_context_writes(context_writes("next_role", "editor"))];
        assert_step_policy_change(
            &ProtocolDefinitionDiff::between(&writing, &changed.build()),
            ProtocolChangeImpact::Strands,
            "which successful output fields update protocol context",
        );

        assert_step_policy_change(
            &ProtocolDefinitionDiff::between(&writing, &baseline()),
            ProtocolChangeImpact::Strands,
            "which successful output fields update protocol context",
        );
    }

    #[test]
    fn a_step_added_is_work_a_session_may_already_have_moved_past() {
        let mut draft = Draft::baseline();
        draft.steps.push(step("review", "OPEN", "noop"));
        let mut actions = facilitator_actions();
        actions.insert(RoleAction::step(step_id("review")));
        draft.roles = vec![role("FACILITATOR", actions)];

        let diff = ProtocolDefinitionDiff::between(&baseline(), &draft.build());

        // Counter-intuitive but the honest reading: a guard waiting on
        // every step to complete would never be satisfied for a session
        // already past the state this one sits in.
        assert!(diff.strands_running_sessions());
        assert_eq!(diff.strand_count(), 1);
    }
}
