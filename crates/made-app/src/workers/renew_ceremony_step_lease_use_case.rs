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
    authorization: Option<Arc<crate::authorization::AuthorizeOperationUseCase>>,
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
            authorization: None,
        }
    }

    #[must_use]
    pub fn with_reauthorization(
        mut self,
        authorization: Arc<crate::authorization::AuthorizeOperationUseCase>,
    ) -> Self {
        self.authorization = Some(authorization);
        self
    }

    pub async fn execute(
        &self,
        ceremony_id: &CeremonyId,
        step_id: &StepId,
        fence: &StepClaimFence,
        owner: &LeaseOwnerId,
        ttl: DurationMs,
    ) -> Result<OffsetDateTime, DomainError> {
        let renewed = self
            .renew(ceremony_id, step_id, fence, owner, ttl, None)
            .await?;
        renewed
            .instance
            .step_record(step_id)
            .and_then(made_core::value_objects::StepExecutionRecord::effective_lease_expires_at)
            .ok_or(DomainError::NotFound { what: "step_lease" })
    }

    pub async fn execute_request(
        &self,
        input: super::RenewCeremonyStepLeaseInput,
    ) -> Result<made_core::entities::ceremony_events::StepLeaseRenewed, DomainError> {
        // Protected calls bind the logical owner to the authenticated producer,
        // even when another principal is otherwise allowed to administer this ceremony.
        if let Some(operation) = crate::services::AuthorizationOperationScope::current() {
            let records = self.stream.records(&input.ceremony_id).await?;
            let source = records
                .iter()
                .find(|record| {
                    matches!(record.event(),
                Some(made_core::entities::CeremonyEvent::StepStarted(started))
                    if started.step_id == input.step_id && started.claim_fence(&input.ceremony_id)
                        .is_ok_and(|fence| fence == input.claim_fence))
                })
                .ok_or(DomainError::NotFound {
                    what: "accepted_step_claim_record",
                })?;
            if source
                .authorization_evidence()
                .is_none_or(|evidence| evidence.principal_id() != operation.principal().id())
            {
                return Err(DomainError::InvariantViolated {
                    reason: "renewal principal does not own the accepted claim",
                });
            }
            let authorize = self
                .authorization
                .as_ref()
                .ok_or(DomainError::InvariantViolated {
                    reason: "delegated renewal requires current claim reauthorization",
                })?;
            let original = made_core::value_objects::AuthorizedOperation::new(
                operation.principal().clone(),
                source
                    .authorization_evidence()
                    .expect("checked evidence")
                    .clone(),
            )?;
            // Both capabilities must still be granted. An outer admission may be
            // cached, so checking only the original claim would miss a separately
            // revoked renewal grant, including on response-loss replay.
            for admitted in [&operation, &original] {
                let request_id = made_core::value_objects::AuthorizationRequestId::new(format!(
                    "renew-check:{}",
                    uuid::Uuid::new_v4()
                ))?;
                if !matches!(
                    authorize.revalidate(admitted, request_id).await?,
                    crate::authorization::AuthorizationGateOutcome::Allowed { .. }
                ) {
                    return Err(DomainError::InvariantViolated {
                        reason: "current authorization refuses lease renewal",
                    });
                }
            }
        }
        let renewed = self
            .renew(
                &input.ceremony_id,
                &input.step_id,
                &input.claim_fence,
                &input.owner,
                input.request.ttl,
                Some(input.request.clone()),
            )
            .await?;
        renewed
            .instance
            .lease_renewal(&input.request.id)
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "accepted_step_lease_renewal",
            })
    }

    async fn renew(
        &self,
        ceremony_id: &CeremonyId,
        step_id: &StepId,
        fence: &StepClaimFence,
        owner: &LeaseOwnerId,
        ttl: DurationMs,
        request: Option<made_core::value_objects::StepLeaseRenewalRequest>,
    ) -> Result<crate::services::LoadedSession, DomainError> {
        let session = self.stream.load(ceremony_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let renewed = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                // A CAS retry must use a fresh clock, never the time before a blocked append.
                let now = self.clock.now();
                if let Some(accepted) = request
                    .as_ref()
                    .and_then(|r| session.instance.lease_renewal(&r.id))
                {
                    let command = CeremonyCommand::RenewStepLease(RenewStepLease {
                        request: request.clone(),
                        step_id: step_id.clone(),
                        claim_fence: fence.clone(),
                        lease_owner_id: owner.clone(),
                        expected_expires_at: accepted.previous_expires_at,
                        expires_at: accepted.expires_at,
                        now,
                    });
                    session.instance.decide(&command, &definition)?;
                    return Ok(Vec::new());
                }
                let key = session
                    .instance
                    .step_record(step_id)
                    .and_then(|record| record.lease())
                    .ok_or(DomainError::NotFound { what: "step_lease" })?
                    .idempotency_key()
                    .clone();
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
                    request: request.clone(),
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
        Ok(renewed)
    }
}
