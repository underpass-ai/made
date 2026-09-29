//! Structural checks for `state_repeat_exhausted` guards.
//!
//! A repeating state with no such exit is legal but can stall: when its
//! last iteration ends without `until` holding, every transition out of
//! it is refused and the ceremony stays there. That is reported as a
//! warning, never an error, so existing definitions stay publishable.
//!
//! The guard names a state rather than a step, so the step-reference
//! checks do not see it. It must name a declared state that repeats,
//! and a transition may carry it only when leaving that same state:
//! exhaustion is observed on the instance's current state, so a guard
//! naming another state could never hold there.

use crate::error::DomainError;
use crate::value_objects::{CeremonyValidationFinding, CeremonyValidationLocus, GuardCondition};

use super::CeremonyDefinitionParts;

pub(super) fn collect(
    parts: &CeremonyDefinitionParts<'_>,
    findings: &mut Vec<CeremonyValidationFinding>,
) {
    for (guard_name, guard) in parts.guards {
        let GuardCondition::StateRepeatExhausted(condition) = guard.condition() else {
            continue;
        };
        let locus = CeremonyValidationLocus::guard(guard_name.clone());
        match parts.states.get(condition.state_id()) {
            None => findings.push(CeremonyValidationFinding::error(
                locus,
                DomainError::NotFound {
                    what: "ceremony_guard.state",
                },
            )),
            Some(state) if state.repeat_policy().is_none() => {
                findings.push(CeremonyValidationFinding::error(
                    locus,
                    DomainError::InvariantViolated {
                        reason: "exhausted-state-repeat guard must reference a repeating state",
                    },
                ));
            }
            Some(_) => {}
        }
    }
    for state in parts.states.values() {
        if state.repeat_policy().is_none() {
            continue;
        }
        let routed = parts.transitions.iter().any(|transition| {
            transition.from() == state.id()
                && transition.required_guards().iter().any(|name| {
                    parts.guards.get(name).is_some_and(|guard| {
                        matches!(
                            guard.condition(),
                            GuardCondition::StateRepeatExhausted(condition)
                                if condition.state_id() == state.id()
                        )
                    })
                })
        });
        if !routed {
            findings.push(CeremonyValidationFinding::warning(
                CeremonyValidationLocus::state(state.id().clone()),
                DomainError::InvariantViolated {
                    reason: "repeating state has no state_repeat_exhausted exit; exhausting its iterations stops the ceremony there",
                },
            ));
        }
    }
    for transition in parts.transitions {
        let names_another_state = transition.required_guards().iter().any(|name| {
            parts.guards.get(name).is_some_and(|guard| {
                matches!(
                    guard.condition(),
                    GuardCondition::StateRepeatExhausted(condition)
                        if condition.state_id() != transition.from()
                )
            })
        });
        if names_another_state {
            findings.push(CeremonyValidationFinding::error(
                CeremonyValidationLocus::transition(
                    transition.from().clone(),
                    transition.trigger().clone(),
                ),
                DomainError::InvariantViolated {
                    reason:
                        "exhausted-state-repeat guard must reference the transition source state",
                },
            ));
        }
    }
}
