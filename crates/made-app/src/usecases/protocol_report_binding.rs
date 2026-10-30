use made_core::value_objects::{
    ProtocolDefinitionDigest, ProtocolId, ProtocolName, ProtocolVersion,
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
pub struct ProtocolReportBinding {
    protocol_id: ProtocolId,
    definition_name: ProtocolName,
    definition_version: ProtocolVersion,
    definition_digest: ProtocolDefinitionDigest,
    bound_definition_digest: Option<ProtocolDefinitionDigest>,
    completed: bool,
}

impl ProtocolReportBinding {
    #[must_use]
    pub fn new(
        protocol_id: ProtocolId,
        definition_name: ProtocolName,
        definition_version: ProtocolVersion,
        definition_digest: ProtocolDefinitionDigest,
        bound_definition_digest: Option<ProtocolDefinitionDigest>,
        completed: bool,
    ) -> Self {
        Self {
            protocol_id,
            definition_name,
            definition_version,
            definition_digest,
            bound_definition_digest,
            completed,
        }
    }

    #[must_use]
    pub fn protocol_id(&self) -> &ProtocolId {
        &self.protocol_id
    }

    #[must_use]
    pub fn definition_name(&self) -> &ProtocolName {
        &self.definition_name
    }

    #[must_use]
    pub fn definition_version(&self) -> &ProtocolVersion {
        &self.definition_version
    }

    #[must_use]
    pub const fn definition_digest(&self) -> &ProtocolDefinitionDigest {
        &self.definition_digest
    }

    #[must_use]
    pub const fn bound_definition_digest(&self) -> Option<&ProtocolDefinitionDigest> {
        self.bound_definition_digest.as_ref()
    }

    /// Whether the session reached a terminal state.
    #[must_use]
    pub const fn completed(&self) -> bool {
        self.completed
    }
}
