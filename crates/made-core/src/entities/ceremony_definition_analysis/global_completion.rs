use super::{
    BTreeSet, CeremonyDefinitionParts, CeremonyTransition, CeremonyValidationFinding,
    CeremonyValidationLocus, DomainError, GuardCondition, StateId,
};

impl CeremonyDefinitionParts<'_> {
    /// Warn about a global completion barrier before an unvisited step.
    ///
    /// Reachability deliberately overestimates what the other guards allow.
    /// A detour that could run the step before this edge suppresses the warning,
    /// including legitimate cycles; analysis never changes execution semantics.
    pub(super) fn collect_global_completion_findings(
        &self,
        findings: &mut Vec<CeremonyValidationFinding>,
    ) {
        let Some(initial) = self.sole_initial_state_id() else {
            return;
        };
        if !self.transition_endpoints_resolve() || !self.transition_guards_resolve() {
            return;
        }
        let before_barrier = self.states_before_global_completion(initial);
        for transition in self.transitions {
            if !before_barrier.contains(transition.from())
                || !self.requires_global_completion(transition)
            {
                continue;
            }
            let downstream = self.states_reachable_from(transition.to());
            if self.steps.values().any(|step| {
                downstream.contains(step.state_id()) && !before_barrier.contains(step.state_id())
            }) {
                findings.push(CeremonyValidationFinding::warning(
                    CeremonyValidationLocus::transition(
                        transition.from().clone(),
                        transition.trigger().clone(),
                    ),
                    DomainError::InvariantViolated {
                        reason: "all_steps_completed is global and waits for downstream steps; use explicit source step_status guards or a state-scoped steps_completed join",
                    },
                ));
            }
        }
    }

    fn requires_global_completion(&self, transition: &CeremonyTransition) -> bool {
        transition.required_guards().iter().any(|name| {
            self.guards
                .get(name)
                .is_some_and(|guard| matches!(guard.condition(), GuardCondition::AllStepsCompleted))
        })
    }

    fn states_before_global_completion(&self, initial: &StateId) -> BTreeSet<StateId> {
        let mut reached = BTreeSet::new();
        let mut pending = vec![initial.clone()];
        while let Some(current) = pending.pop() {
            if !reached.insert(current.clone()) {
                continue;
            }
            for transition in self.transitions.iter().filter(|transition| {
                transition.from() == &current && !self.requires_global_completion(transition)
            }) {
                pending.push(transition.to().clone());
            }
        }
        reached
    }
}
