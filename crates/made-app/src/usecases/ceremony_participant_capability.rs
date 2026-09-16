/// What a seated role may do beyond the stages it owns.
///
/// The live agenda is the only thing a participant can take part in
/// without owning a stage, so these are the only two: everything else
/// a role does in a designed ceremony it does because a stage or the
/// final approval named it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CeremonyParticipantCapability {
    RequestIntervention,
    RespondToIntervention,
}

impl CeremonyParticipantCapability {
    /// The role action this capability becomes in the definition.
    #[must_use]
    pub const fn as_action(self) -> &'static str {
        match self {
            Self::RequestIntervention => "request_intervention",
            Self::RespondToIntervention => "respond_to_intervention",
        }
    }
}
