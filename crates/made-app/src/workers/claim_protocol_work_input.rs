use made_core::value_objects::{
    AuditActorKind, DurationMs, LeaseOwnerId, ProtocolId, ProtocolInstancePageLimit,
};

/// One bounded discovery and claim request for the reference worker host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimProtocolWorkInput {
    after: Option<ProtocolId>,
    limit: ProtocolInstancePageLimit,
    lease_owner_id: LeaseOwnerId,
    lease_ttl: DurationMs,
    actor_kind: AuditActorKind,
}

impl ClaimProtocolWorkInput {
    #[must_use]
    pub const fn new(
        after: Option<ProtocolId>,
        limit: ProtocolInstancePageLimit,
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
    pub const fn after(&self) -> Option<&ProtocolId> {
        self.after.as_ref()
    }

    /// Returns the same bounded request with a new keyset cursor.
    #[must_use]
    pub fn with_after(&self, after: Option<ProtocolId>) -> Self {
        Self {
            after,
            limit: self.limit,
            lease_owner_id: self.lease_owner_id.clone(),
            lease_ttl: self.lease_ttl,
            actor_kind: self.actor_kind,
        }
    }

    #[must_use]
    pub const fn limit(&self) -> ProtocolInstancePageLimit {
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
