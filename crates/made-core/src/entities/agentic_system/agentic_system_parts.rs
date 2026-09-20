use std::collections::BTreeMap;

use crate::value_objects::{
    CeremonyComposition, CollaborationLink, LogicalParticipant, ParticipantId,
    RequestedExecutionProfile, SupervisionPolicy, SystemCeremonyId, SystemRole, SystemRoleId,
};

use super::AgenticSystem;

/// Everything the analysis reads, borrowed once.
///
/// The checks run over the same design from a dozen angles, and each
/// of them wanting four of these fields would either pass four
/// arguments or take the whole aggregate and reach in. One borrow with
/// a name keeps the checks free functions over data rather than
/// methods that could quietly start changing it.
#[derive(Debug, Clone, Copy)]
pub struct AgenticSystemParts<'design> {
    pub(crate) integrator: &'design SystemRoleId,
    pub(crate) roles: &'design BTreeMap<SystemRoleId, SystemRole>,
    pub(crate) participants: &'design BTreeMap<ParticipantId, LogicalParticipant>,
    pub(crate) topology: &'design [CollaborationLink],
    pub(crate) profiles: &'design BTreeMap<SystemRoleId, RequestedExecutionProfile>,
    pub(crate) ceremonies: &'design BTreeMap<SystemCeremonyId, CeremonyComposition>,
    pub(crate) supervision: &'design SupervisionPolicy,
}

impl<'design> AgenticSystemParts<'design> {
    #[must_use]
    pub fn of(design: &'design AgenticSystem) -> Self {
        Self {
            integrator: design.integrator(),
            roles: design.roles(),
            participants: design.participants(),
            topology: design.topology(),
            profiles: design.profiles(),
            ceremonies: design.ceremonies(),
            supervision: design.supervision(),
        }
    }

    /// What each composition must wait for, with bounded loops' back
    /// edges cut.
    #[must_use]
    pub fn blocking_dependencies(&self) -> BTreeMap<SystemCeremonyId, Vec<SystemCeremonyId>> {
        super::dependency_order::blocking(self.ceremonies)
    }

    /// Which logical participant sits in one system role, if any does.
    #[must_use]
    pub fn participant_of_role(&self, role: &SystemRoleId) -> Option<&'design ParticipantId> {
        self.participants
            .values()
            .find(|participant| participant.role() == role)
            .map(LogicalParticipant::id)
    }
}
