use made_core::entities::{AuditRecord, CeremonyDefinition, CeremonyInstance};

/// Authoritative data needed to render one already-authorized ceremony reply.
#[derive(Debug, Clone)]
pub struct EmbeddedCeremonyProjectionData {
    instance: CeremonyInstance,
    definition: CeremonyDefinition,
    records: Vec<AuditRecord>,
}

impl EmbeddedCeremonyProjectionData {
    pub(super) fn new(
        instance: CeremonyInstance,
        definition: CeremonyDefinition,
        records: Vec<AuditRecord>,
    ) -> Self {
        Self {
            instance,
            definition,
            records,
        }
    }

    #[must_use]
    pub const fn instance(&self) -> &CeremonyInstance {
        &self.instance
    }

    #[must_use]
    pub const fn definition(&self) -> &CeremonyDefinition {
        &self.definition
    }

    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }
}
