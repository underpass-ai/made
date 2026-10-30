use made_core::entities::ProtocolDefinition;
use made_core::value_objects::{
    AuditActorId, AuditActorKind, DurationMs, LeaseOwnerId, ProtocolContext, ProtocolId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunProtocolInput {
    id: ProtocolId,
    definition: ProtocolDefinition,
    context: ProtocolContext,
    lease_owner_id: LeaseOwnerId,
    lease_ttl: DurationMs,
    actor_id: AuditActorId,
    actor_kind: AuditActorKind,
}

impl RunProtocolInput {
    /// How long a step lease lasts when the caller asked for no
    /// particular length.
    ///
    /// The engine runs the step itself here, so the lease only has to
    /// outlive one handler call; a minute is generous for that and
    /// short enough that a crashed run does not hold a step until
    /// someone notices. Declared next to the input that carries it, as
    /// [`ReadProtocolEventsInput::DEFAULT_LIMIT`] is, so an adapter
    /// reads the number instead of choosing one: the same omission
    /// used to mean thirty seconds in process and sixty over the wire.
    ///
    /// [`ReadProtocolEventsInput::DEFAULT_LIMIT`]: super::ReadProtocolEventsInput::DEFAULT_LIMIT
    pub const DEFAULT_LEASE_TTL_MS: u64 = 60_000;

    #[must_use]
    pub fn new(
        id: ProtocolId,
        definition: ProtocolDefinition,
        context: ProtocolContext,
        lease_owner_id: LeaseOwnerId,
        lease_ttl: DurationMs,
        actor_id: impl Into<String>,
        actor_kind: AuditActorKind,
    ) -> Self {
        Self {
            id,
            definition,
            context,
            lease_owner_id,
            lease_ttl,
            actor_id: AuditActorId::new(actor_id),
            actor_kind,
        }
    }

    #[must_use]
    pub fn id(&self) -> &ProtocolId {
        &self.id
    }

    #[must_use]
    pub fn definition(&self) -> &ProtocolDefinition {
        &self.definition
    }

    #[must_use]
    pub fn context(&self) -> &ProtocolContext {
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
        ProtocolId,
        ProtocolDefinition,
        ProtocolContext,
        LeaseOwnerId,
        DurationMs,
        AuditActorId,
        AuditActorKind,
    ) {
        (
            self.id,
            self.definition,
            self.context,
            self.lease_owner_id,
            self.lease_ttl,
            self.actor_id,
            self.actor_kind,
        )
    }
}
