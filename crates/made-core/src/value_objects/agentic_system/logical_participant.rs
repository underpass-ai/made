use serde::{Deserialize, Serialize};

use super::{ParticipantBindingPolicy, ParticipantId, ParticipantKind, SystemRoleId};

/// Somebody or something the system expects to take part, described by
/// what it must be able to do rather than by who it is.
///
/// A logical participant is not an agent. The same design run twice
/// can be played by different agents, and a design that named one
/// would be a deployment pretending to be a description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalParticipant {
    id: ParticipantId,
    role: SystemRoleId,
    kind: ParticipantKind,
    #[serde(default)]
    binding: ParticipantBindingPolicy,
}

impl LogicalParticipant {
    #[must_use]
    pub const fn new(
        id: ParticipantId,
        role: SystemRoleId,
        kind: ParticipantKind,
        binding: ParticipantBindingPolicy,
    ) -> Self {
        Self {
            id,
            role,
            kind,
            binding,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &ParticipantId {
        &self.id
    }

    #[must_use]
    pub const fn role(&self) -> &SystemRoleId {
        &self.role
    }

    #[must_use]
    pub const fn kind(&self) -> ParticipantKind {
        self.kind
    }

    #[must_use]
    pub const fn binding(&self) -> &ParticipantBindingPolicy {
        &self.binding
    }
}
