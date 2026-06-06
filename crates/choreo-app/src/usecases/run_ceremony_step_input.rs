use choreo_core::value_objects::{
    CeremonyId, CeremonyName, CeremonyVersion, DurationMs, IdempotencyKey, LeaseOwnerId, RoleId,
    StepId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCeremonyStepInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) definition_name: CeremonyName,
    pub(crate) definition_version: CeremonyVersion,
    pub(crate) role_id: RoleId,
    pub(crate) step_id: StepId,
    pub(crate) lease_owner_id: LeaseOwnerId,
    pub(crate) idempotency_key: IdempotencyKey,
    pub(crate) lease_ttl: DurationMs,
}

impl RunCeremonyStepInput {
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        definition_name: CeremonyName,
        definition_version: CeremonyVersion,
        role_id: RoleId,
        step_id: StepId,
        lease_owner_id: LeaseOwnerId,
        idempotency_key: IdempotencyKey,
        lease_ttl: DurationMs,
    ) -> Self {
        Self {
            instance_id,
            definition_name,
            definition_version,
            role_id,
            step_id,
            lease_owner_id,
            idempotency_key,
            lease_ttl,
        }
    }
}
