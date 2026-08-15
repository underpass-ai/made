use super::{CeremonyDefinitionDigest, CeremonyName, CeremonyVersion};

/// A content-verified transition between two definition identity schemes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyDefinitionDigestMigration {
    definition_name: CeremonyName,
    definition_version: CeremonyVersion,
    source: CeremonyDefinitionDigest,
    destination: CeremonyDefinitionDigest,
}

impl CeremonyDefinitionDigestMigration {
    pub(crate) fn verified(
        definition_name: CeremonyName,
        definition_version: CeremonyVersion,
        source: CeremonyDefinitionDigest,
        destination: CeremonyDefinitionDigest,
    ) -> Self {
        Self {
            definition_name,
            definition_version,
            source,
            destination,
        }
    }

    #[must_use]
    pub fn definition_name(&self) -> &CeremonyName {
        &self.definition_name
    }

    #[must_use]
    pub fn definition_version(&self) -> &CeremonyVersion {
        &self.definition_version
    }

    #[must_use]
    pub fn source(&self) -> CeremonyDefinitionDigest {
        self.source
    }

    #[must_use]
    pub fn destination(&self) -> CeremonyDefinitionDigest {
        self.destination
    }
}
