use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::MemoryCapability;

/// What a memory backend can actually do.
///
/// A named set rather than a row of flags: capabilities are negotiated
/// with something on the other side of a boundary, and a set survives
/// one side learning a new trick without the other having to be
/// rebuilt to hear about it.
///
/// A backend that claims a capability it does not have fails the
/// conformance suite. A backend that claims none is still a legitimate
/// backend, and the honest shape of "no memory configured".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MemoryCapabilities(BTreeSet<MemoryCapability>);

impl MemoryCapabilities {
    /// A backend that does nothing and says so.
    #[must_use]
    pub fn none() -> Self {
        Self(BTreeSet::new())
    }

    #[must_use]
    pub fn all() -> Self {
        Self::none()
            .with(MemoryCapability::Remembering)
            .with(MemoryCapability::Recalling)
            .with(MemoryCapability::AnsweringQuestions)
            .with(MemoryCapability::TravellingInTime)
            .with(MemoryCapability::KeepingEvidence)
            .with(MemoryCapability::KeepingReasons)
            .with(MemoryCapability::FollowingReasons)
    }

    #[must_use]
    pub fn with(mut self, capability: MemoryCapability) -> Self {
        self.0.insert(capability);
        self
    }

    #[must_use]
    pub fn has(&self, capability: MemoryCapability) -> bool {
        self.0.contains(&capability)
    }

    #[must_use]
    pub fn remembers(&self) -> bool {
        self.has(MemoryCapability::Remembering)
    }

    #[must_use]
    pub fn recalls(&self) -> bool {
        self.has(MemoryCapability::Recalling)
    }

    #[must_use]
    pub fn answers_questions(&self) -> bool {
        self.has(MemoryCapability::AnsweringQuestions)
    }

    #[must_use]
    pub fn travels_in_time(&self) -> bool {
        self.has(MemoryCapability::TravellingInTime)
    }

    #[must_use]
    pub fn keeps_evidence(&self) -> bool {
        self.has(MemoryCapability::KeepingEvidence)
    }

    #[must_use]
    pub fn keeps_reasons(&self) -> bool {
        self.has(MemoryCapability::KeepingReasons)
    }

    #[must_use]
    pub fn follows_reasons(&self) -> bool {
        self.has(MemoryCapability::FollowingReasons)
    }

    pub fn iter(&self) -> impl Iterator<Item = MemoryCapability> + '_ {
        self.0.iter().copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
