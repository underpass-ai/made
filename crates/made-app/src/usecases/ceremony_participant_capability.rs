use made_core::error::DomainError;

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
    /// Every capability there is, in the order a reader meets them.
    pub const ALL: [Self; 2] = [Self::RequestIntervention, Self::RespondToIntervention];

    /// The role action this capability becomes in the definition.
    #[must_use]
    pub const fn as_action(self) -> &'static str {
        match self {
            Self::RequestIntervention => "request_intervention",
            Self::RespondToIntervention => "respond_to_intervention",
        }
    }

    /// Read one back from the word a caller wrote.
    ///
    /// The word is the action itself, so the vocabulary is stated
    /// once: a surface that accepted a different spelling would seat
    /// a role that could do nothing.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.as_action() == value.trim())
            .ok_or_else(|| DomainError::InvalidDocument {
                reason: format!(
                    "participant capability `{value}` is not one of `{}` or `{}`",
                    Self::RequestIntervention.as_action(),
                    Self::RespondToIntervention.as_action()
                ),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_word_per_capability_read_the_same_way_it_is_written() {
        for capability in CeremonyParticipantCapability::ALL {
            assert_eq!(
                CeremonyParticipantCapability::parse(capability.as_action()),
                Ok(capability)
            );
        }
    }

    #[test]
    fn a_word_that_is_not_one_of_them_says_which_they_are() {
        let error = CeremonyParticipantCapability::parse("observe")
            .expect_err("`observe` is not a capability");

        let DomainError::InvalidDocument { reason } = error else {
            panic!("a word the vocabulary does not have is the caller's to fix");
        };
        assert!(reason.contains("observe"), "{reason}");
        assert!(reason.contains("request_intervention"), "{reason}");
        assert!(reason.contains("respond_to_intervention"), "{reason}");
    }
}
