//! [`CouncilSelector`] — how the caller of `RunCouncilDecision`
//! identifies the council that owns the deliberation.
//!
//! Mirrors the proto-side `oneof selector`: either a stable
//! [`CouncilId`] (preferred when the caller already knows it) or the
//! [`Specialty`] the council is registered under (preferred when the
//! caller's view of the world is domain-shaped — "ask the triage
//! council", not "ask council 0x42").

use crate::value_objects::{CouncilId, Specialty};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CouncilSelector {
    ById(CouncilId),
    BySpecialty(Specialty),
}

impl CouncilSelector {
    /// Convenience constructor for the by-specialty arm.
    #[must_use]
    pub fn by_specialty(specialty: Specialty) -> Self {
        Self::BySpecialty(specialty)
    }

    /// Convenience constructor for the by-id arm.
    #[must_use]
    pub fn by_id(council_id: CouncilId) -> Self {
        Self::ById(council_id)
    }
}
