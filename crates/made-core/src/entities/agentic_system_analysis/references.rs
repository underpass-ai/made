//! Does every name in the design refer to something the design has?
//!
//! The cheapest class of defect and the one that makes every other
//! check meaningless: an analysis that reasoned about a role nobody
//! declared would be reasoning about nothing.

use crate::entities::AgenticSystemParts;
use crate::value_objects::{
    AgenticSystemValidationFinding, AgenticSystemValidationLocus, LogicalParticipant, SystemRoleKind,
};

pub(super) fn collect(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    participants_name_declared_roles(parts, findings);
    profiles_name_declared_roles(parts, findings);
    topology_names_declared_participants(parts, findings);
    the_integrator_is_an_integrator_with_somebody_in_it(parts, findings);
    supervised_roles_are_declared(parts, findings);
}

fn participants_name_declared_roles(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for participant in parts.participants.values() {
        if !parts.roles.contains_key(participant.role()) {
            findings.push(AgenticSystemValidationFinding::refusal(
                AgenticSystemValidationLocus::participant(participant.id().clone()),
                format!("role `{}` is not declared by this system", participant.role()),
            ));
        }
    }
}

fn profiles_name_declared_roles(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for role in parts.profiles.keys() {
        if !parts.roles.contains_key(role) {
            findings.push(AgenticSystemValidationFinding::refusal(
                AgenticSystemValidationLocus::profile(role.clone()),
                format!("role `{role}` is not declared by this system"),
            ));
        }
    }
}

fn topology_names_declared_participants(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for link in parts.topology {
        for endpoint in [link.from(), link.to()] {
            if !parts.participants.contains_key(endpoint) {
                findings.push(AgenticSystemValidationFinding::refusal(
                    AgenticSystemValidationLocus::topology(
                        link.from().clone(),
                        link.to().clone(),
                    ),
                    format!("participant `{endpoint}` is not declared by this system"),
                ));
            }
        }
    }
}

/// The integrator is the one role the system cannot be described
/// without: it is who the system answers to and who it asks. A design
/// whose integrator is an observer, or is nobody, has no outside.
fn the_integrator_is_an_integrator_with_somebody_in_it(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    let locus = AgenticSystemValidationLocus::role(parts.integrator.clone());
    let Some(role) = parts.roles.get(parts.integrator) else {
        findings.push(AgenticSystemValidationFinding::refusal(
            locus,
            format!(
                "the integrator role `{}` is not declared by this system",
                parts.integrator
            ),
        ));
        return;
    };
    if !role.kind().drives_the_system() {
        findings.push(AgenticSystemValidationFinding::refusal(
            locus.clone(),
            format!(
                "the integrator role `{}` is declared as `{}`, not `{}`",
                parts.integrator,
                role.kind(),
                SystemRoleKind::Integrator
            ),
        ));
    }
    if parts.participant_of_role(parts.integrator).is_none() {
        findings.push(AgenticSystemValidationFinding::refusal(
            locus,
            format!(
                "no participant plays the integrator role `{}`",
                parts.integrator
            ),
        ));
    }
}

fn supervised_roles_are_declared(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for role in parts.supervision.role_actions().keys() {
        if !parts.roles.contains_key(role) {
            findings.push(AgenticSystemValidationFinding::refusal(
                AgenticSystemValidationLocus::role(role.clone()),
                format!("role `{role}` is not declared by this system"),
            ));
        }
    }
    for rule in parts.supervision.independence() {
        for role in [rule.reviewer(), rule.reviewed()] {
            if !parts.roles.contains_key(role) {
                findings.push(AgenticSystemValidationFinding::refusal(
                    AgenticSystemValidationLocus::independence(
                        rule.reviewer().clone(),
                        rule.reviewed().clone(),
                    ),
                    format!("role `{role}` is not declared by this system"),
                ));
            }
        }
    }
}

/// Every participant playing one role.
pub(super) fn participants_of_role<'design>(
    parts: AgenticSystemParts<'design>,
    role: &'design crate::value_objects::SystemRoleId,
) -> impl Iterator<Item = &'design LogicalParticipant> {
    parts
        .participants
        .values()
        .filter(move |participant| participant.role() == role)
}
