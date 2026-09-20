//! Can anybody who might play this role actually do what the role's
//! work asks for?
//!
//! The design states requirements against roles and supplies against
//! participants, and the two are written in different places by
//! different people. A requirement nobody meets is the defect that
//! turns into a skipped ceremony at run time, so it is worth refusing
//! before publication rather than discovering during a run.

use crate::entities::AgenticSystemParts;
use crate::value_objects::{AgenticSystemValidationFinding, AgenticSystemValidationLocus};

use super::references;

pub(super) fn collect(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for (role, profile) in parts.profiles {
        if !parts.roles.contains_key(role) {
            // Already reported as an unknown reference; saying it
            // twice in two vocabularies helps nobody.
            continue;
        }
        for capability in profile.required_capabilities() {
            let supplied = references::participants_of_role(parts, role)
                .any(|participant| participant.binding().supplies(capability));
            if !supplied {
                findings.push(AgenticSystemValidationFinding::refusal(
                    AgenticSystemValidationLocus::profile(role.clone()),
                    format!(
                        "role `{role}` requires capability `{capability}`, which no participant \
                         playing it declares"
                    ),
                ));
            }
        }
    }
}
