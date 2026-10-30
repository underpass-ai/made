//! [`UnreadableProtocolPublication`] — a catalogue entry this engine
//! cannot hand back as a publication.
//!
//! A publication is only handed out when its definition still yields
//! the digest it was published with. One that does not — a row
//! re-encoded by a migration that did not reseal it, a payload this
//! engine cannot decode — is not dropped from the catalogue: its name,
//! version and recorded digest are still true, and saying so is how an
//! operator finds out it needs repairing.

use crate::error::DomainError;
use crate::value_objects::{ProtocolDefinitionDigest, ProtocolName, ProtocolVersion};

#[derive(Debug, Clone, PartialEq)]
pub struct UnreadableProtocolPublication {
    name: ProtocolName,
    version: ProtocolVersion,
    recorded_digest: Option<ProtocolDefinitionDigest>,
    defect: DomainError,
}

impl UnreadableProtocolPublication {
    /// `recorded_digest` is the digest the store holds for the entry,
    /// when it could be read at all.
    #[must_use]
    pub const fn new(
        name: ProtocolName,
        version: ProtocolVersion,
        recorded_digest: Option<ProtocolDefinitionDigest>,
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
    pub const fn name(&self) -> &ProtocolName {
        &self.name
    }

    #[must_use]
    pub const fn version(&self) -> &ProtocolVersion {
        &self.version
    }

    #[must_use]
    pub const fn recorded_digest(&self) -> Option<ProtocolDefinitionDigest> {
        self.recorded_digest
    }

    /// Why the entry cannot be handed back.
    #[must_use]
    pub const fn defect(&self) -> &DomainError {
        &self.defect
    }
}
