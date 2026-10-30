use made_core::entities::protocol_commands::StartStep;
use made_core::entities::{ProtocolCommand, ProtocolDefinition};
use made_core::error::DomainError;
use made_core::value_objects::{AuditActorKind, RoleId, StepLease};

use super::{RunProtocolStepInput, RunProtocolStepUseCase};
use crate::services::{session_facts, ConflictPolicy, LoadedSession};

impl RunProtocolStepUseCase {
    pub(super) async fn claim_step(
        &self,
        session: LoadedSession,
        definition: &ProtocolDefinition,
        input: &RunProtocolStepInput,
        requested_role_id: Option<RoleId>,
        actor_kind: AuditActorKind,
    ) -> Result<LoadedSession, DomainError> {
        let now = self.clock.now();
        let lease = StepLease::acquire(
            input.lease_owner_id.clone(),
            input.idempotency_key.clone(),
            now,
            input.lease_ttl,
        )?;
        let claim = ProtocolCommand::StartStep(StartStep {
            role_id: requested_role_id,
            step_id: input.step_id.clone(),
            lease,
            now,
            max_parallel_ceiling: self.max_parallel_ceiling,
            budget_reservation_id: None,
        });
        self.stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&claim, definition)?;
                let actor = session_facts::step_started_seat(&events, actor_kind)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await
    }
}
