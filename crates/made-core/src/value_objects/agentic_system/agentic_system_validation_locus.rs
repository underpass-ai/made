use std::fmt;

use serde::{Deserialize, Serialize};

use crate::value_objects::{GuardName, InputName, RoleId};

use super::{ParticipantId, SystemCeremonyId, SystemRoleId};

/// The exact element of an agentic system design a finding refers to.
///
/// A design has more kinds of element than a ceremony definition and
/// they are easy to confuse — a role of the system is not a role of a
/// composed ceremony — so the locus names which vocabulary it is
/// speaking. Without it, "role `reviewer` is unknown" points at two
/// different places.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AgenticSystemValidationLocus {
    /// The design as a whole, when no narrower element applies.
    System,
    Role {
        role: SystemRoleId,
    },
    Participant {
        participant: ParticipantId,
    },
    Ceremony {
        ceremony: SystemCeremonyId,
    },
    /// One seat of one composed ceremony's own definition.
    CeremonyRole {
        ceremony: SystemCeremonyId,
        role: RoleId,
    },
    CeremonyInput {
        ceremony: SystemCeremonyId,
        input: InputName,
    },
    Topology {
        from: ParticipantId,
        to: ParticipantId,
    },
    Profile {
        role: SystemRoleId,
    },
    Independence {
        reviewer: SystemRoleId,
        reviewed: SystemRoleId,
    },
    Guard {
        ceremony: SystemCeremonyId,
        guard: GuardName,
    },
}

impl AgenticSystemValidationLocus {
    #[must_use]
    pub const fn role(role: SystemRoleId) -> Self {
        Self::Role { role }
    }

    #[must_use]
    pub const fn participant(participant: ParticipantId) -> Self {
        Self::Participant { participant }
    }

    #[must_use]
    pub const fn ceremony(ceremony: SystemCeremonyId) -> Self {
        Self::Ceremony { ceremony }
    }

    #[must_use]
    pub const fn ceremony_role(ceremony: SystemCeremonyId, role: RoleId) -> Self {
        Self::CeremonyRole { ceremony, role }
    }

    #[must_use]
    pub const fn ceremony_input(ceremony: SystemCeremonyId, input: InputName) -> Self {
        Self::CeremonyInput { ceremony, input }
    }

    #[must_use]
    pub const fn topology(from: ParticipantId, to: ParticipantId) -> Self {
        Self::Topology { from, to }
    }

    #[must_use]
    pub const fn profile(role: SystemRoleId) -> Self {
        Self::Profile { role }
    }

    #[must_use]
    pub const fn independence(reviewer: SystemRoleId, reviewed: SystemRoleId) -> Self {
        Self::Independence { reviewer, reviewed }
    }

    #[must_use]
    pub const fn guard(ceremony: SystemCeremonyId, guard: GuardName) -> Self {
        Self::Guard { ceremony, guard }
    }
}

/// Where a finding points, said in words.
///
/// The serialized form names the element for a machine; this names it
/// for whoever has to fix the design.
impl fmt::Display for AgenticSystemValidationLocus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System => formatter.write_str("the system"),
            Self::Role { role } => write!(formatter, "role `{role}`"),
            Self::Participant { participant } => write!(formatter, "participant `{participant}`"),
            Self::Ceremony { ceremony } => write!(formatter, "ceremony `{ceremony}`"),
            Self::CeremonyRole { ceremony, role } => {
                write!(formatter, "seat `{role}` of ceremony `{ceremony}`")
            }
            Self::CeremonyInput { ceremony, input } => {
                write!(formatter, "input `{input}` of ceremony `{ceremony}`")
            }
            Self::Topology { from, to } => {
                write!(formatter, "the link from `{from}` to `{to}`")
            }
            Self::Profile { role } => write!(formatter, "the requested profile of role `{role}`"),
            Self::Independence { reviewer, reviewed } => write!(
                formatter,
                "the independence of role `{reviewer}` from role `{reviewed}`"
            ),
            Self::Guard { ceremony, guard } => {
                write!(formatter, "guard `{guard}` of ceremony `{ceremony}`")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_role_and_a_ceremony_seat_read_differently() {
        assert_eq!(
            AgenticSystemValidationLocus::role(SystemRoleId::new("reviewer").unwrap()).to_string(),
            "role `reviewer`"
        );
        assert_eq!(
            AgenticSystemValidationLocus::ceremony_role(
                SystemCeremonyId::new("review").unwrap(),
                RoleId::new("reviewer").unwrap(),
            )
            .to_string(),
            "seat `reviewer` of ceremony `review`"
        );
        assert_eq!(
            AgenticSystemValidationLocus::System.to_string(),
            "the system"
        );
    }
}
