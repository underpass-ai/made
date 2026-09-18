use std::collections::{BTreeMap, BTreeSet};

use crate::value_objects::{CeremonyState, CeremonyTransition, StateId};

/// Cyclic strongly connected components in deterministic state-id order.
pub(super) fn cyclic_components(
    states: &BTreeMap<StateId, CeremonyState>,
    transitions: &[CeremonyTransition],
) -> Vec<BTreeSet<StateId>> {
    let mut assigned = BTreeSet::new();
    let mut components = Vec::new();
    for state_id in states.keys() {
        if assigned.contains(state_id) {
            continue;
        }
        let forward = reachable_from(state_id, transitions);
        let component = forward
            .into_iter()
            .filter(|candidate| reachable_from(candidate, transitions).contains(state_id))
            .collect::<BTreeSet<_>>();
        assigned.extend(component.iter().cloned());
        let self_loop = component.len() == 1
            && transitions
                .iter()
                .any(|transition| transition.from() == state_id && transition.to() == state_id);
        if component.len() > 1 || self_loop {
            components.push(component);
        }
    }
    components
}

fn reachable_from(initial: &StateId, transitions: &[CeremonyTransition]) -> BTreeSet<StateId> {
    let mut reached = BTreeSet::new();
    let mut pending = vec![initial.clone()];
    while let Some(current) = pending.pop() {
        if !reached.insert(current.clone()) {
            continue;
        }
        pending.extend(
            transitions
                .iter()
                .filter(|transition| transition.from() == &current)
                .map(|transition| transition.to().clone()),
        );
    }
    reached
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{CeremonyState, TransitionTrigger};

    fn state(raw: &str) -> StateId {
        StateId::new(raw).unwrap()
    }

    fn edge(from: &str, to: &str) -> CeremonyTransition {
        CeremonyTransition::new(
            state(from),
            state(to),
            TransitionTrigger::new(format!("{from}_{to}")).unwrap(),
            Vec::new(),
        )
        .unwrap()
    }

    #[test]
    fn finds_self_loops_and_multi_state_components_but_not_diamonds() {
        let states = ["a", "b", "c", "d", "e", "f"]
            .into_iter()
            .map(|id| (state(id), CeremonyState::intermediate(state(id))))
            .collect();
        let transitions = vec![
            edge("a", "b"),
            edge("a", "c"),
            edge("b", "d"),
            edge("c", "d"),
            edge("d", "e"),
            edge("e", "d"),
            edge("f", "f"),
        ];

        assert_eq!(
            cyclic_components(&states, &transitions),
            [
                BTreeSet::from([state("d"), state("e")]),
                BTreeSet::from([state("f")])
            ]
        );
    }
}
