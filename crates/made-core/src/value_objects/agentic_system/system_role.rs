use serde::{Deserialize, Serialize};

use super::{Responsibility, SystemRoleId, SystemRoleKind};

/// A business role the system is described in terms of.
///
/// Roles are the stable vocabulary: participants change between runs,
/// ceremonies are composed and recomposed, and what stays is who is
/// answerable for what.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SystemRole {
    id: SystemRoleId,
    responsibility: Responsibility,
    kind: SystemRoleKind,
}

impl SystemRole {
    #[must_use]
    pub const fn new(
        id: SystemRoleId,
        responsibility: Responsibility,
        kind: SystemRoleKind,
    ) -> Self {
        Self {
            id,
            responsibility,
            kind,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &SystemRoleId {
        &self.id
    }

    #[must_use]
    pub const fn responsibility(&self) -> &Responsibility {
        &self.responsibility
    }

    #[must_use]
    pub const fn kind(&self) -> SystemRoleKind {
        self.kind
    }
}
