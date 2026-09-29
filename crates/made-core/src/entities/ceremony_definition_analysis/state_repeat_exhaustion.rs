//! Structural checks for `state_repeat_exhausted` guards.
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
