use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::value_objects::{AgenticSystemDigest, AgenticSystemRevision};

/// A design as a caller reads it back.
///
/// The digest travels with it because a design without one can be
/// copied, edited and offered back as though it were the same design.
/// The YAML rendering is not here: it is a wire shape, and the
/// adapters render it from this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgenticSystemView {
    system: AgenticSystem,
    digest: AgenticSystemDigest,
}

impl AgenticSystemView {
    pub fn of(system: AgenticSystem) -> Result<Self, DomainError> {
        let digest = system.digest()?;
        Ok(Self { system, digest })
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
    pub fn revision(&self) -> AgenticSystemRevision {
        self.system.revision()
    }
}
