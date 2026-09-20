use serde::{Deserialize, Serialize};

/// Where one accepted claim stands when a report was taken.
///
/// A phase, not a status: the step record says what the engine holds,
/// and this says what that means for the claim an operator is asking
/// about. It belongs to the domain because succession decides against
/// it — a claim that is still `Live` or only `Expired` needs an
/// explicit disposition before the work can be handed to a successor,
/// and a decision cannot reach into the application layer to learn it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeremonyClaimPhase {
    Live,
    Expired,
    Retired,
    Completed,
    Failed,
}

impl CeremonyClaimPhase {
    /// Whether this claim is still outstanding, so a handoff has to say
    /// what became of it.
    #[must_use]
    pub const fn is_outstanding(self) -> bool {
        matches!(self, Self::Live | Self::Expired)
    }
}
