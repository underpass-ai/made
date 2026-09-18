//! Reachability analysis for structurally valid ceremony graphs.

use std::collections::BTreeSet;

use crate::error::DomainError;
use crate::value_objects::{
    CeremonyState, CeremonyValidationFinding, CeremonyValidationLocus, StateId,
};

use super::CeremonyDefinitionParts;

impl CeremonyDefinitionParts<'_> {
    /// Reachability defects are reported as warnings.
    ///
    /// They describe a ceremony that can stall rather than one that is
    /// structurally impossible, and promoting them to errors would
    /// reject definitions that construct today. Whether any of them
    /// graduates to an error is a separate, deliberate decision.
    ///
    /// Analysis is skipped when the graph is not sound enough to walk:
    /// structural errors are reported first and reachability noise on
    /// top of them helps nobody.
    pub(super) fn collect_reachability_findings(
        &self,
        findings: &mut Vec<CeremonyValidationFinding>,
    ) {
        let Some(initial) = self.sole_initial_state_id() else {
            return;
        };
        if !self.transition_endpoints_resolve() {
            return;
        }

        if !self.states.values().any(CeremonyState::is_terminal) {
            findings.push(CeremonyValidationFinding::warning(
                CeremonyValidationLocus::Definition,
                DomainError::InvariantViolated {
                    reason: "ceremony definition has no terminal state",
                },
            ));
            return;
        }

        let reachable = self.states_reachable_from(initial);
        let can_finish = self.states_that_reach_a_terminal();
        for state_id in self.states.keys() {
            if !reachable.contains(state_id) {
                findings.push(CeremonyValidationFinding::warning(
                    CeremonyValidationLocus::state(state_id.clone()),
                    DomainError::InvariantViolated {
                        reason: "ceremony state is unreachable from the initial state",
                    },
                ));
            } else if !can_finish.contains(state_id) {
                findings.push(CeremonyValidationFinding::warning(
                    CeremonyValidationLocus::state(state_id.clone()),
                    DomainError::InvariantViolated {
                        reason: "no terminal state is reachable from this ceremony state",
                    },
                ));
            }
        }
    }

    pub(super) fn sole_initial_state_id(&self) -> Option<&StateId> {
        let mut initial_states = self.states.iter().filter(|(_, state)| state.is_initial());
        let (state_id, _) = initial_states.next()?;
        match initial_states.next() {
            Some(_) => None,
            None => Some(state_id),
        }
    }

    pub(super) fn transition_endpoints_resolve(&self) -> bool {
        self.transitions.iter().all(|transition| {
            self.states.contains_key(transition.from()) && self.states.contains_key(transition.to())
        })
    }

    pub(super) fn states_reachable_from(&self, initial: &StateId) -> BTreeSet<StateId> {
        let mut reached = BTreeSet::new();
        let mut pending = vec![initial.clone()];
        while let Some(current) = pending.pop() {
            if !reached.insert(current.clone()) {
                continue;
            }
            for transition in self
                .transitions
                .iter()
                .filter(|transition| transition.from() == &current)
            {
                pending.push(transition.to().clone());
            }
        }
        reached
    }

    fn states_that_reach_a_terminal(&self) -> BTreeSet<StateId> {
        let mut can_finish = self
            .states
            .iter()
            .filter(|(_, state)| state.is_terminal())
            .map(|(state_id, _)| state_id.clone())
            .collect::<BTreeSet<_>>();

        let mut grew = true;
        while grew {
            grew = false;
            for transition in self.transitions {
                if can_finish.contains(transition.to()) && !can_finish.contains(transition.from()) {
                    can_finish.insert(transition.from().clone());
                    grew = true;
                }
            }
        }
        can_finish
    }
}
