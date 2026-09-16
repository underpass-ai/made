use made_core::value_objects::{
    CeremonyDefinitionDigest, CeremonyId, CeremonyName, CeremonyVersion,
};

/// Which definition one session in a report was running, and whether it
/// finished.
///
/// The digest is the definition's own, computed from what the report
/// read; `bound_definition_digest` is the published digest the session
/// recorded when it was started from a version, and it is absent for a
/// session started from a document. Reading the two together is how a
/// reader tells "this is the definition it ran" from "this is a
/// definition of the same name".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyReportBinding {
    ceremony_id: CeremonyId,
    definition_name: CeremonyName,
    definition_version: CeremonyVersion,
    definition_digest: CeremonyDefinitionDigest,
    bound_definition_digest: Option<CeremonyDefinitionDigest>,
    completed: bool,
}

impl CeremonyReportBinding {
    #[must_use]
    pub fn new(
        ceremony_id: CeremonyId,
        definition_name: CeremonyName,
        definition_version: CeremonyVersion,
        definition_digest: CeremonyDefinitionDigest,
        bound_definition_digest: Option<CeremonyDefinitionDigest>,
        completed: bool,
    ) -> Self {
        Self {
            ceremony_id,
            definition_name,
            definition_version,
            definition_digest,
            bound_definition_digest,
            completed,
        }
    }

    #[must_use]
    pub fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
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
    pub const fn definition_digest(&self) -> &CeremonyDefinitionDigest {
        &self.definition_digest
    }

    #[must_use]
    pub const fn bound_definition_digest(&self) -> Option<&CeremonyDefinitionDigest> {
        self.bound_definition_digest.as_ref()
    }

    /// Whether the session reached a terminal state.
    #[must_use]
    pub const fn completed(&self) -> bool {
        self.completed
    }
}
