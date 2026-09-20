use std::collections::BTreeSet;

use made_core::value_objects::{Capability, Specialty};
use serde::Deserialize;

/// What a host says it can put behind one logical participant.
///
/// The design states what a participant must be able to do; only the
/// host knows what it actually has. This is the host answering, and
/// it is the whole of the answer: MADE compares the offer against the
/// design and never fills a gap in it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantOffer {
    specialty: Specialty,
    #[serde(default)]
    capabilities: BTreeSet<Capability>,
}

impl ParticipantOffer {
    #[must_use]
    pub fn new(specialty: Specialty, capabilities: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            specialty,
            capabilities: capabilities.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn specialty(&self) -> &Specialty {
        &self.specialty
    }

    #[must_use]
    pub const fn capabilities(&self) -> &BTreeSet<Capability> {
        &self.capabilities
    }

    /// The first capability this offer does not cover, if any.
    #[must_use]
    pub fn missing<'need>(
        &self,
        needed: impl IntoIterator<Item = &'need Capability>,
    ) -> Option<&'need Capability> {
        needed
            .into_iter()
            .find(|capability| !self.capabilities.contains(*capability))
    }
}
