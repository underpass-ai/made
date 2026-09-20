use std::collections::BTreeMap;

use serde::Serialize;

use crate::value_objects::{
    AgenticSystemId, AttentionPolicy, CeremonyComposition, CollaborationLink, LogicalParticipant,
    ParticipantId, RequestedExecutionProfile, SupervisionPolicy, SystemCeremonyId, SystemPurpose,
    SystemRole, SystemRoleId,
};

use super::AgenticSystem;

/// What the design says, with nothing about when it was said.
///
/// The digest must identify the design and only the design. Folding in
/// the revision would make a save that changed nothing produce a new
/// identity; folding in the timestamps would make the same document
/// written twice look like two different systems; and folding in the
/// lifecycle would change a design's identity at the moment it was
/// published, which is precisely when it must stop changing.
#[derive(Debug, Serialize)]
pub(super) struct AgenticSystemContent<'design> {
    id: &'design AgenticSystemId,
    purpose: &'design SystemPurpose,
    integrator: &'design SystemRoleId,
    roles: &'design BTreeMap<SystemRoleId, SystemRole>,
    participants: &'design BTreeMap<ParticipantId, LogicalParticipant>,
    topology: &'design [CollaborationLink],
    profiles: &'design BTreeMap<SystemRoleId, RequestedExecutionProfile>,
    ceremonies: &'design BTreeMap<SystemCeremonyId, CeremonyComposition>,
    supervision: &'design SupervisionPolicy,
    attention: &'design AttentionPolicy,
}

impl<'design> AgenticSystemContent<'design> {
    pub(super) fn of(design: &'design AgenticSystem) -> Self {
        Self {
            id: design.id(),
            purpose: design.purpose(),
            integrator: design.integrator(),
            roles: design.roles(),
            participants: design.participants(),
            topology: design.topology(),
            profiles: design.profiles(),
            ceremonies: design.ceremonies(),
            supervision: design.supervision(),
            attention: design.attention(),
        }
    }
}
