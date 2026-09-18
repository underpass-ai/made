use made_core::value_objects::RoleId;

/// Whether a step claim names its role or asks the aggregate to resolve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepRoleResolution {
    Explicit,
    Automatic,
}

impl StepRoleResolution {
    pub(crate) fn requested(self, anchor: &RoleId) -> Option<RoleId> {
        match self {
            Self::Explicit => Some(anchor.clone()),
            Self::Automatic => None,
        }
    }
}
