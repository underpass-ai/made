use crate::error::DomainError;
use crate::value_objects::{
    CeremonyStep, CeremonyTransition, CeremonyValidationFinding, CeremonyValidationLocus,
    GuardCondition, StateExecution,
};

use super::CeremonyDefinitionParts;

pub(super) fn collect(
    parts: &CeremonyDefinitionParts<'_>,
    findings: &mut Vec<CeremonyValidationFinding>,
) {
    for step in parts
        .steps
        .values()
        .filter(|step| step.aggregation().is_some())
    {
        let locus = CeremonyValidationLocus::step(step.id().clone());
        let Some(destination) = parts.states.get(step.state_id()) else {
            continue;
        };
        if destination.is_initial() {
            push_error(
                findings,
                &locus,
                "aggregate step cannot belong to the initial state",
            );
        }
        if destination.execution() != StateExecution::Sequential {
            push_error(
                findings,
                &locus,
                "aggregate step destination state must be sequential",
            );
        }
        let first_in_state = parts.step_order.iter().find(|candidate| {
            parts
                .steps
                .get(*candidate)
                .is_some_and(|candidate| candidate.state_id() == step.state_id())
        });
        if first_in_state != Some(step.id()) {
            push_error(
                findings,
                &locus,
                "aggregate step must be first in its destination state",
            );
        }
        if step
            .handler_config()
            .attributes()
            .get("see_prior")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            push_error(findings, &locus, "aggregate step requires see_prior: true");
        }

        let incoming = parts
            .transitions
            .iter()
            .filter(|transition| transition.to() == step.state_id())
            .collect::<Vec<_>>();
        if incoming.len() != 1 {
            push_error(
                findings,
                &locus,
                "aggregate step requires exactly one predecessor state",
            );
            continue;
        }
        let transition = incoming[0];
        if transition.from() == transition.to() {
            push_error(
                findings,
                &locus,
                "aggregate step predecessor must differ from its destination",
            );
            continue;
        }
        let Some(predecessor) = parts.states.get(transition.from()) else {
            continue;
        };
        if predecessor.execution() != StateExecution::Concurrent {
            push_error(
                findings,
                &locus,
                "aggregate step requires a concurrent predecessor state",
            );
            continue;
        }
        let siblings = parts
            .steps
            .values()
            .filter(|candidate| candidate.state_id() == predecessor.id())
            .collect::<Vec<_>>();
        if !requires_every_sibling(parts, transition, &siblings) {
            push_error(
                findings,
                &locus,
                "aggregate step requires an all-siblings predecessor join",
            );
        }
    }
}

fn requires_every_sibling(
    parts: &CeremonyDefinitionParts<'_>,
    transition: &CeremonyTransition,
    steps: &[&CeremonyStep],
) -> bool {
    let required = transition
        .required_guards()
        .iter()
        .filter_map(|name| parts.guards.get(name))
        .collect::<Vec<_>>();
    steps.iter().all(|step| {
        required.iter().any(|guard| {
            matches!(
                guard.condition(),
                GuardCondition::StepStatus { step_id, status }
                    if step_id == step.id() && status.is_success()
            )
        })
    })
}

fn push_error(
    findings: &mut Vec<CeremonyValidationFinding>,
    locus: &CeremonyValidationLocus,
    reason: &'static str,
) {
    findings.push(CeremonyValidationFinding::error(
        locus.clone(),
        DomainError::InvariantViolated { reason },
    ));
}
