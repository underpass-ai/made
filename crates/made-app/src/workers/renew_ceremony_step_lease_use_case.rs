use crate::services::{session_facts, ConflictPolicy, SessionStream};
use crate::usecases::ResolveCeremonyDefinitionUseCase;
use made_core::entities::ceremony_commands::RenewStepLease;
use made_core::entities::CeremonyCommand;
use made_core::error::DomainError;
use made_core::ports::ClockPort;
use made_core::value_objects::{
    AuditActorKind, CeremonyId, DurationMs, LeaseOwnerId, StepClaimFence, StepId, StepLease,
};
use std::sync::Arc;
use time::OffsetDateTime;

/// Journal CAS renewal, preserving the original producer identity and reservation.
pub struct RenewCeremonyStepLeaseUseCase {
    stream: Arc<SessionStream>,
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for RenewCeremonyStepLeaseUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RenewCeremonyStepLeaseUseCase")
            .finish_non_exhaustive()
    }
}

impl RenewCeremonyStepLeaseUseCase {
    pub fn new(
        stream: Arc<SessionStream>,
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            stream,
            definitions,
            clock,
        }
    }

    pub async fn execute(
        &self,
        ceremony_id: &CeremonyId,
        step_id: &StepId,
        fence: &StepClaimFence,
        owner: &LeaseOwnerId,
        ttl: DurationMs,
    ) -> Result<OffsetDateTime, DomainError> {
        let session = self.stream.load(ceremony_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let key = session
            .instance
            .step_record(step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?
            .lease()
            .ok_or(DomainError::NotFound { what: "step_lease" })?
            .idempotency_key()
            .clone();
        let renewed = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                // A CAS retry must use a fresh clock, never the time before a blocked append.
                let now = self.clock.now();
                let expected_expires_at = session
                    .instance
                    .step_record(step_id)
                    .and_then(
                        made_core::value_objects::StepExecutionRecord::effective_lease_expires_at,
                    )
                    .ok_or(DomainError::NotFound { what: "step_lease" })?;
                let requested =
                    StepLease::acquire(owner.clone(), key.clone(), now, ttl)?.expires_at();
                let deadline = session
                    .instance
                    .step_deadlines()
                    .get(step_id)
                    .map(made_core::value_objects::StepDeadline::at)
                    .into_iter()
                    .chain(
                        session
                            .instance
                            .state_deadline()
                            .map(made_core::value_objects::StateDeadline::at),
                    )
                    .chain(
                        session
                            .instance
                            .ceremony_deadline()
                            .map(made_core::value_objects::CeremonyDeadline::at),
                    )
                    .min();
                let expires_at = deadline
                    .map_or(requested, |at| requested.min(at))
                    .max(expected_expires_at);
                let command = CeremonyCommand::RenewStepLease(RenewStepLease {
                    step_id: step_id.clone(),
                    claim_fence: fence.clone(),
                    lease_owner_id: owner.clone(),
                    expected_expires_at,
                    expires_at,
                    now,
                });
                let events = session.instance.decide(&command, &definition)?;
                let actor = session_facts::party(owner.as_str(), AuditActorKind::Engine)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await?;
        renewed
            .instance
            .step_record(step_id)
            .and_then(made_core::value_objects::StepExecutionRecord::effective_lease_expires_at)
            .ok_or(DomainError::NotFound { what: "step_lease" })
    }
}
