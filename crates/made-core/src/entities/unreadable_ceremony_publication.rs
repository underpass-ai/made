//! [`UnreadableCeremonyPublication`] — a catalogue entry this engine
//! cannot hand back as a publication.
//!
//! A publication is only handed out when its definition still yields
//! the digest it was published with. One that does not — a row
//! re-encoded by a migration that did not reseal it, a payload this
//! engine cannot decode — is not dropped from the catalogue: its name,
//! version and recorded digest are still true, and saying so is how an
//! operator finds out it needs repairing.

use crate::error::DomainError;
use crate::value_objects::{CeremonyDefinitionDigest, CeremonyName, CeremonyVersion};

#[derive(Debug, Clone, PartialEq)]
pub struct UnreadableCeremonyPublication {
    name: CeremonyName,
    version: CeremonyVersion,
    recorded_digest: Option<CeremonyDefinitionDigest>,
    defect: DomainError,
}

impl UnreadableCeremonyPublication {
    /// `recorded_digest` is the digest the store holds for the entry,
    /// when it could be read at all.
    #[must_use]
    pub const fn new(
        name: CeremonyName,
        version: CeremonyVersion,
        recorded_digest: Option<CeremonyDefinitionDigest>,
        defect: DomainError,
    ) -> Self {
        Self {
            name,
            version,
            recorded_digest,
            defect,
        }
    }

    #[must_use]
    pub const fn name(&self) -> &CeremonyName {
        &self.name
    }

    #[must_use]
    pub const fn version(&self) -> &CeremonyVersion {
        &self.version
    }

    #[must_use]
    pub const fn recorded_digest(&self) -> Option<CeremonyDefinitionDigest> {
        self.recorded_digest
    }

    /// Why the entry cannot be handed back.
    #[must_use]
    pub const fn defect(&self) -> &DomainError {
        &self.defect
    }
}
