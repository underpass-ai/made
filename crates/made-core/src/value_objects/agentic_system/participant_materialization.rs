use serde::{Deserialize, Serialize};

use crate::value_objects::Specialty;

use super::UnavailabilityReason;

/// What a run found when it went looking for somebody to play one
/// logical participant.
///
/// `Unavailable` is a first-class answer. A run that could not find a
/// participant says so and skips the ceremonies that needed it; it
/// never stands in for the missing party, because a design proved by
/// a stand-in has been proved about nothing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ParticipantMaterialization {
    Bound { specialty: Specialty },
    Unavailable { reason: UnavailabilityReason },
}

impl ParticipantMaterialization {
    #[must_use]
    pub const fn bound(specialty: Specialty) -> Self {
        Self::Bound { specialty }
    }

    #[must_use]
    pub const fn unavailable(reason: UnavailabilityReason) -> Self {
        Self::Unavailable { reason }
    }

    #[must_use]
    pub const fn is_bound(&self) -> bool {
        matches!(self, Self::Bound { .. })
    }

    #[must_use]
    pub const fn specialty(&self) -> Option<&Specialty> {
        match self {
            Self::Bound { specialty } => Some(specialty),
            Self::Unavailable { .. } => None,
        }
    }

    #[must_use]
    pub const fn reason(&self) -> Option<&UnavailabilityReason> {
        match self {
            Self::Bound { .. } => None,
            Self::Unavailable { reason } => Some(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unavailable_participant_carries_what_was_missing() {
        let unavailable = ParticipantMaterialization::unavailable(
            UnavailabilityReason::new("no host supplies capability `browsing`").unwrap(),
        );

        assert!(!unavailable.is_bound());
        assert!(unavailable.specialty().is_none());
        assert!(unavailable.reason().is_some());
    }
}
