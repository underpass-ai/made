//! [`CeremonyTranscript`] — the ordered record of every intervention a
//! ceremony has produced so far.
//!
//! The transcript is what makes a ceremony a conversation rather than a
//! sequence of monologues: the context store accumulates it and the
//! engine hands it to each step so later interventions can build on the
//! earlier ones.

use serde::{Deserialize, Serialize};

use super::CeremonyStepContribution;

/// The ordered list of contributions accumulated during a ceremony.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyTranscript(Vec<CeremonyStepContribution>);

impl CeremonyTranscript {
    /// Build a transcript from an ordered list of contributions.
    #[must_use]
    pub fn new(contributions: Vec<CeremonyStepContribution>) -> Self {
        Self(contributions)
    }

    /// An empty transcript — a ceremony that has produced nothing yet.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Return this transcript extended with `contribution` appended last.
    #[must_use]
    pub fn appended(mut self, contribution: CeremonyStepContribution) -> Self {
        self.0.push(contribution);
        self
    }

    /// The contributions in the order they were produced.
    #[must_use]
    pub fn contributions(&self) -> &[CeremonyStepContribution] {
        &self.0
    }

    /// Whether the transcript holds no contributions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The number of contributions recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{RoleId, StepId, StepOutput};

    fn contribution(step: &str, role: &str) -> CeremonyStepContribution {
        CeremonyStepContribution::new(
            StepId::new(step).unwrap(),
            RoleId::new(role).unwrap(),
            StepOutput::empty(),
        )
    }

    #[test]
    fn empty_transcript_has_no_contributions() {
        let transcript = CeremonyTranscript::empty();

        assert!(transcript.is_empty());
        assert_eq!(transcript.len(), 0);
        assert!(transcript.contributions().is_empty());
    }

    #[test]
    fn appended_preserves_order() {
        let transcript = CeremonyTranscript::empty()
            .appended(contribution("open_room", "FACILITATOR"))
            .appended(contribution("customer_story", "CUSTOMER_ADVOCATE"));

        assert_eq!(transcript.len(), 2);
        assert!(!transcript.is_empty());
        assert_eq!(
            transcript.contributions()[0].step_id().as_str(),
            "open_room"
        );
        assert_eq!(
            transcript.contributions()[1].step_id().as_str(),
            "customer_story"
        );
    }
}
