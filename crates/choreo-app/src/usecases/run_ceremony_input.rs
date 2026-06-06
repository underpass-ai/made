use choreo_core::entities::CeremonyDefinition;
use choreo_core::value_objects::{CeremonyContext, CeremonyId, DurationMs, LeaseOwnerId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCeremonyInput {
    id: CeremonyId,
    definition: CeremonyDefinition,
    context: CeremonyContext,
    lease_owner_id: LeaseOwnerId,
    lease_ttl: DurationMs,
}

impl RunCeremonyInput {
    #[must_use]
    pub fn new(
        id: CeremonyId,
        definition: CeremonyDefinition,
        context: CeremonyContext,
        lease_owner_id: LeaseOwnerId,
        lease_ttl: DurationMs,
    ) -> Self {
        Self {
            id,
            definition,
            context,
            lease_owner_id,
            lease_ttl,
        }
    }

    #[must_use]
    pub fn id(&self) -> &CeremonyId {
        &self.id
    }

    #[must_use]
    pub fn definition(&self) -> &CeremonyDefinition {
        &self.definition
    }

    #[must_use]
    pub fn context(&self) -> &CeremonyContext {
        &self.context
    }

    #[must_use]
    pub fn lease_owner_id(&self) -> &LeaseOwnerId {
        &self.lease_owner_id
    }

    #[must_use]
    pub fn lease_ttl(&self) -> DurationMs {
        self.lease_ttl
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        CeremonyId,
        CeremonyDefinition,
        CeremonyContext,
        LeaseOwnerId,
        DurationMs,
    ) {
        (
            self.id,
            self.definition,
            self.context,
            self.lease_owner_id,
            self.lease_ttl,
        )
    }
}
