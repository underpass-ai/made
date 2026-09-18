use made_core::value_objects::{
    AuditActorKind, CeremonyId, DurationMs, IdempotencyKey, LeaseOwnerId, RoleId, StepId,
};

use super::step_role_resolution::StepRoleResolution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartCeremonyStepInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) role_id: RoleId,
    /// What kind of party is running it.
    ///
    /// Carried, never worked out. The engine sees a seat and cannot
    /// see what fills it.
    pub(crate) role_kind: AuditActorKind,
    pub(crate) step_id: StepId,
    pub(crate) lease_owner_id: LeaseOwnerId,
    pub(crate) idempotency_key: IdempotencyKey,
    pub(crate) lease_ttl: DurationMs,
    pub(crate) role_resolution: StepRoleResolution,
}

impl StartCeremonyStepInput {
    /// How long a claimed step stays claimed when the caller asked for
    /// no particular length.
    ///
    /// Longer than the engine's own default by an order of magnitude,
    /// and deliberately: this lease covers work a host does outside the
    /// engine, where nobody can see progress, so it has to outlive a
    /// human-paced turnaround rather than one handler call.
    pub const DEFAULT_LEASE_TTL_MS: u64 = 300_000;

    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        role_id: RoleId,
        role_kind: AuditActorKind,
        step_id: StepId,
        lease_owner_id: LeaseOwnerId,
        idempotency_key: IdempotencyKey,
        lease_ttl: DurationMs,
    ) -> Self {
        Self {
            instance_id,
            role_id,
            role_kind,
            step_id,
            lease_owner_id,
            idempotency_key,
            lease_ttl,
            role_resolution: StepRoleResolution::Explicit,
        }
    }

    /// Let the aggregate resolve the role from the session observed by each
    /// optimistic attempt.
    ///
    /// `role_id()` remains the requested/fallback anchor for compatibility;
    /// the role actually accepted is the one sealed by `StepStarted`.
    #[must_use]
    pub fn with_automatic_role_resolution(mut self) -> Self {
        self.role_resolution = StepRoleResolution::Automatic;
        self
    }

    pub(crate) fn requested_role_id(&self) -> Option<RoleId> {
        self.role_resolution.requested(&self.role_id)
    }

    #[must_use]
    pub fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }

    #[must_use]
    pub fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    #[must_use]
    pub const fn role_kind(&self) -> AuditActorKind {
        self.role_kind
    }

    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }

    #[must_use]
    pub fn lease_owner_id(&self) -> &LeaseOwnerId {
        &self.lease_owner_id
    }

    #[must_use]
    pub fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }

    #[must_use]
    pub const fn lease_ttl(&self) -> DurationMs {
        self.lease_ttl
    }
}
