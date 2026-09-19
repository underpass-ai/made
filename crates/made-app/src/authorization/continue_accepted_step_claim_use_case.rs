use std::sync::Arc;

use made_core::entities::CeremonyEvent;
use made_core::ports::ClockPort;
use made_core::value_objects::{AuthorizationAction, AuthorizedOperation};
use made_core::DomainError;

use super::{
    AcceptedStepCompletion, AuthorizationGateOutcome, AuthorizeOperationUseCase,
    ContinueAcceptedCeremonyWorkUseCase,
};
use crate::services::{LoadedSession, SessionStream};

#[derive(Clone)]
pub struct ContinueAcceptedStepClaimUseCase {
    stream: Arc<SessionStream>,
    continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
    clock: Arc<dyn ClockPort>,
    reauthorize: Option<Arc<AuthorizeOperationUseCase>>,
}

impl ContinueAcceptedStepClaimUseCase {
    #[must_use]
    pub fn new(
        stream: Arc<SessionStream>,
        continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            stream,
            continuation,
            clock,
            reauthorize: None,
        }
    }

    #[must_use]
    pub fn with_reauthorization(mut self, authorize: Arc<AuthorizeOperationUseCase>) -> Self {
        self.reauthorize = Some(authorize);
        self
    }

    pub async fn execute(
        &self,
        input: AcceptedStepCompletion,
    ) -> Result<AuthorizedOperation, DomainError> {
        self.execute_action(input, AuthorizationAction::CompleteCeremonyStep)
            .await
    }

    /// Continue a durably accepted claim while its fence and lease remain current.
    pub async fn execute_renewal(
        &self,
        input: AcceptedStepCompletion,
    ) -> Result<AuthorizedOperation, DomainError> {
        self.execute_action(input, AuthorizationAction::RenewCeremonyStepLease)
            .await
    }

    async fn execute_action(
        &self,
        input: AcceptedStepCompletion,
        action: AuthorizationAction,
    ) -> Result<AuthorizedOperation, DomainError> {
        let records = self.stream.records(&input.ceremony_id).await?;
        let session = SessionStream::fold_records(&records)?;
        let source = records
            .iter()
            .rev()
            .find(|record| match record.event() {
                Some(CeremonyEvent::StepStarted(started)) if started.step_id == input.step_id => {
                    started
                        .claim_fence(&input.ceremony_id)
                        .is_ok_and(|fence| fence == input.claim_fence)
                }
                _ => false,
            })
            .ok_or(DomainError::NotFound {
                what: "accepted_step_claim_record",
            })?;
        if action == AuthorizationAction::RenewCeremonyStepLease {
            self.reauthorize_claim(source, &input).await?;
        }
        if let Some(existing) = self
            .continuation
            .existing_for(
                source,
                input.request_id.clone(),
                action,
                input.target_digest.clone(),
                Some(&input.principal),
            )
            .await?
        {
            return Ok(existing);
        }
        require_live_claim(&session, &input, self.clock.now())?;
        self.continuation
            .execute_for(
                source,
                input.request_id,
                action,
                input.target_digest,
                Some(&input.principal),
            )
            .await
    }

    async fn reauthorize_claim(
        &self,
        source: &made_core::entities::AuditRecord,
        input: &AcceptedStepCompletion,
    ) -> Result<(), DomainError> {
        let authorize = self
            .reauthorize
            .as_ref()
            .ok_or(DomainError::InvariantViolated {
                reason: "lease renewal requires current claim reauthorization",
            })?;
        let evidence =
            source
                .authorization_evidence()
                .cloned()
                .ok_or(DomainError::InvariantViolated {
                    reason: "accepted step claim has no authorization evidence",
                })?;
        let operation = AuthorizedOperation::new(input.principal.clone(), evidence)?;
        let request_id = made_core::value_objects::AuthorizationRequestId::new(format!(
            "renew-check:{}",
            input.request_id.as_str()
        ))?;
        match authorize.revalidate(&operation, request_id).await? {
            AuthorizationGateOutcome::Allowed { .. } => Ok(()),
            AuthorizationGateOutcome::Denied { .. } | AuthorizationGateOutcome::Expired { .. } => {
                Err(DomainError::InvariantViolated {
                    reason: "current authorization refuses lease renewal",
                })
            }
        }
    }
}

fn require_live_claim(
    session: &LoadedSession,
    input: &AcceptedStepCompletion,
    now: time::OffsetDateTime,
) -> Result<(), DomainError> {
    if session.instance.step_claim_fence(&input.step_id)? != input.claim_fence {
        return Err(DomainError::InvariantViolated {
            reason: "accepted step completion fence is no longer current",
        });
    }
    let record = session
        .instance
        .step_record(&input.step_id)
        .ok_or(DomainError::NotFound {
            what: "ceremony_instance.step_record",
        })?;
    if !record.has_live_lease_at(now) {
        return Err(DomainError::InvariantViolated {
            reason: "accepted step completion lease is no longer live",
        });
    }
    Ok(())
}

impl std::fmt::Debug for ContinueAcceptedStepClaimUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContinueAcceptedStepClaimUseCase")
            .finish_non_exhaustive()
    }
}
