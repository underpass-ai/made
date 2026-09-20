//! [`PublishedAgenticSystem`] — a design fixed to a content identity.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{
    AgenticSystemDigest, AgenticSystemId, AgenticSystemRevision, SystemPin,
};

use super::AgenticSystem;

/// A sealed revision of an agentic system, and the digest that says
/// which bytes it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedAgenticSystem {
    system: AgenticSystem,
    digest: AgenticSystemDigest,
    #[serde(with = "time::serde::rfc3339")]
    published_at: OffsetDateTime,
}

impl PublishedAgenticSystem {
    /// Fix a design to its content identity.
    pub fn seal(system: AgenticSystem, published_at: OffsetDateTime) -> Result<Self, DomainError> {
        let digest = system.digest()?;
        Ok(Self {
            system,
            digest,
            published_at,
        })
    }

    #[must_use]
    pub const fn system(&self) -> &AgenticSystem {
        &self.system
    }

    #[must_use]
    pub const fn digest(&self) -> AgenticSystemDigest {
        self.digest
    }

    #[must_use]
    pub const fn published_at(&self) -> OffsetDateTime {
        self.published_at
    }

    #[must_use]
    pub fn id(&self) -> &AgenticSystemId {
        self.system.id()
    }

    #[must_use]
    pub fn revision(&self) -> AgenticSystemRevision {
        self.system.revision()
    }

    /// How a run names this exact design.
    #[must_use]
    pub fn pin(&self) -> SystemPin {
        SystemPin::new(self.id().clone(), self.revision(), self.digest)
    }

    #[must_use]
    pub fn into_system(self) -> AgenticSystem {
        self.system
    }
}
