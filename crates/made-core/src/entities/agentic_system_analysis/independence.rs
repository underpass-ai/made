//! Is the review actually independent?
//!
//! A rule between two roles only bites where the two roles meet, and
//! where they meet is one composed ceremony. Two participants are not
//! independent when they are the same participant, and — the case that
//! matters for agents — when they are two names for one opinion, which
//! is what an independence group declares.

use std::collections::BTreeSet;

use crate::entities::AgenticSystemParts;
use crate::value_objects::{
    AgenticSystemValidationFinding, AgenticSystemValidationLocus, IndependenceRule,
    ParticipantId, SystemCeremonyId, SystemRoleId,
};

use super::references;

pub(super) fn collect(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for rule in parts.supervision.independence() {
        for (ceremony, composition) in parts.ceremonies {
            let seated: BTreeSet<&ParticipantId> = composition.role_bindings().values().collect();
            check_one_meeting(parts, rule, ceremony, &seated, findings);
        }
    }
}

fn check_one_meeting(
    parts: AgenticSystemParts<'_>,
    rule: &IndependenceRule,
    ceremony: &SystemCeremonyId,
    seated: &BTreeSet<&ParticipantId>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    let reviewers = seated_of_role(parts, rule.reviewer(), seated);
    let reviewed = seated_of_role(parts, rule.reviewed(), seated);
    if reviewers.is_empty() || reviewed.is_empty() {
        return;
    }
    for reviewer in &reviewers {
        for subject in &reviewed {
            if let Some(reason) = why_not_independent(parts, reviewer, subject) {
                findings.push(AgenticSystemValidationFinding::refusal(
                    AgenticSystemValidationLocus::independence(
                        rule.reviewer().clone(),
                        rule.reviewed().clone(),
                    ),
                    format!("in ceremony `{ceremony}`, {reason}"),
                ));
            }
        }
    }
}

fn seated_of_role<'design>(
    parts: AgenticSystemParts<'design>,
    role: &'design SystemRoleId,
    seated: &BTreeSet<&ParticipantId>,
) -> Vec<&'design ParticipantId> {
    references::participants_of_role(parts, role)
        .map(crate::value_objects::LogicalParticipant::id)
        .filter(|participant| seated.contains(participant))
        .collect()
}

fn why_not_independent(
    parts: AgenticSystemParts<'_>,
    reviewer: &ParticipantId,
    reviewed: &ParticipantId,
) -> Option<String> {
    if reviewer == reviewed {
        return Some(format!(
            "participant `{reviewer}` would review its own work"
        ));
    }
    let group = parts
        .participants
        .get(reviewer)?
        .binding()
        .independence_group()?;
    let other = parts
        .participants
        .get(reviewed)?
        .binding()
        .independence_group()?;
    (group == other).then(|| {
        format!(
            "participants `{reviewer}` and `{reviewed}` are both in independence group `{group}`"
        )
    })
}
