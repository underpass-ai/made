use made_core::entities::{AuditRecord, ProtocolDefinition, ProtocolInstance};

/// Authoritative data needed to render one already-authorized protocol reply.
#[derive(Debug, Clone)]
pub struct EmbeddedProtocolProjectionData {
    instance: ProtocolInstance,
    definition: ProtocolDefinition,
    records: Vec<AuditRecord>,
}

impl EmbeddedProtocolProjectionData {
    pub(super) fn new(
        instance: ProtocolInstance,
        definition: ProtocolDefinition,
        records: Vec<AuditRecord>,
    ) -> Self {
        Self {
            instance,
            definition,
            records,
        }
    }

    #[must_use]
    pub const fn instance(&self) -> &ProtocolInstance {
        &self.instance
    }

    #[must_use]
    pub const fn definition(&self) -> &ProtocolDefinition {
        &self.definition
    }

    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }
}
