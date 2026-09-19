use made_core::value_objects::{
    AuditActorKind, CeremonyId, CeremonyInstancePageLimit, DurationMs, LeaseOwnerId,
};

/// One bounded discovery and claim request for the reference worker host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimCeremonyWorkInput {
    after: Option<CeremonyId>,
    limit: CeremonyInstancePageLimit,
    lease_owner_id: LeaseOwnerId,
    lease_ttl: DurationMs,
    actor_kind: AuditActorKind,
}

impl ClaimCeremonyWorkInput {
    #[must_use]
    pub const fn new(
        after: Option<CeremonyId>,
        limit: CeremonyInstancePageLimit,
        lease_owner_id: LeaseOwnerId,
        lease_ttl: DurationMs,
        actor_kind: AuditActorKind,
    ) -> Self {
        Self {
            after,
            limit,
            lease_owner_id,
            lease_ttl,
            actor_kind,
        }
    }

    #[must_use]
    pub const fn after(&self) -> Option<&CeremonyId> {
        self.after.as_ref()
    }

    #[must_use]
    pub const fn limit(&self) -> CeremonyInstancePageLimit {
        self.limit
    }

    #[must_use]
    pub const fn lease_owner_id(&self) -> &LeaseOwnerId {
        &self.lease_owner_id
    }

    #[must_use]
    pub const fn lease_ttl(&self) -> DurationMs {
        self.lease_ttl
    }

    #[must_use]
    pub const fn actor_kind(&self) -> AuditActorKind {
        self.actor_kind
    }
}
