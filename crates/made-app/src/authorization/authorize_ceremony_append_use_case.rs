//! Keep long-running admissions live without allowing revoked work to claim again.
use std::sync::Arc;

use made_core::entities::{AuditFact, AuditRecord, CeremonyEvent};
use made_core::ports::ClockPort;
use made_core::value_objects::{
    AuthorizationAction, AuthorizationRequestId, AuthorizationTargetDigest, AuthorizedOperation,
    CeremonyId, StreamVersion,
};
use made_core::DomainError;

use super::{AuthorizeOperationUseCase, ContinueAcceptedCeremonyWorkUseCase};
use crate::services::SessionStream;

pub struct AuthorizeCeremonyAppendUseCase {
    authorize: Arc<AuthorizeOperationUseCase>,
    continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for AuthorizeCeremonyAppendUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizeCeremonyAppendUseCase")
            .finish_non_exhaustive()
    }
}

impl AuthorizeCeremonyAppendUseCase {
    #[must_use]
    pub fn new(
        authorize: Arc<AuthorizeOperationUseCase>,
        continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            authorize,
            continuation,
            clock,
        }
    }

    pub async fn execute(
        &self,
        stream: &SessionStream,
        ceremony_id: &CeremonyId,
        version: StreamVersion,
        facts: &[AuditFact],
        operation: AuthorizedOperation,
    ) -> Result<AuthorizedOperation, DomainError> {
        let new_claim = operation.evidence().action() == AuthorizationAction::RunCeremony
            && facts
                .iter()
                .any(|fact| matches!(fact.event, CeremonyEvent::StepStarted(_)));
        if !new_claim && operation.evidence().is_live_at(self.clock.now()) {
            return Ok(operation);
        }
        let target = serde_json::to_vec(&(
            operation.evidence().decision_id(),
            ceremony_id,
            version,
            facts
                .iter()
                .map(|fact| (&fact.event_id, &fact.event, &fact.actor, fact.occurred_at))
                .collect::<Vec<_>>(),
        ))
        .map_err(|_| DomainError::InvariantViolated {
            reason: "ceremony append admission cannot be canonicalized",
        })?;
        let digest = AuthorizationTargetDigest::for_bytes(&target);
        let request_id = AuthorizationRequestId::new(format!("append-{}", digest.as_str()))?;
        if let Some(first) = facts.first() {
            if matches!(
                first.event,
                CeremonyEvent::StepCompleted(_)
                    | CeremonyEvent::StepFailed(_)
                    | CeremonyEvent::LateStepResultObserved(_)
            ) {
                let records = stream.records(ceremony_id).await?;
                let source = records
                    .iter()
                    .rev()
                    .find(|record| claim_matches(record, &first.event))
                    .ok_or(DomainError::NotFound {
                        what: "accepted_step_claim",
                    })?;
                return self
                    .continuation
                    .execute_for(
                        source,
                        request_id,
                        AuthorizationAction::CompleteCeremonyStep,
                        digest,
                        Some(operation.principal()),
                    )
                    .await;
            }
        }
        let outcome = self.authorize.revalidate(&operation, request_id).await?;
        let evidence = outcome
            .evidence()
            .cloned()
            .ok_or(DomainError::InvariantViolated {
                reason: "current authorization refuses new ceremony work",
            })?;
        AuthorizedOperation::new(operation.principal().clone(), evidence)
    }
}

fn claim_matches(record: &AuditRecord, result: &CeremonyEvent) -> bool {
    let Some(CeremonyEvent::StepStarted(started)) = record.event() else {
        return false;
    };
    let coordinates = (
        &started.step_id,
        started.state_visit(),
        started.state_iteration(),
        started.iteration,
        started.attempt,
    );
    match result {
        CeremonyEvent::StepCompleted(event) => {
            coordinates
                == (
                    &event.step_id,
                    event.state_visit(),
                    event.state_iteration(),
                    event.iteration,
                    event.attempt,
                )
        }
        CeremonyEvent::StepFailed(event) => {
            coordinates
                == (
                    &event.step_id,
                    event.state_visit(),
                    event.state_iteration(),
                    event.iteration,
                    event.attempt,
                )
        }
        CeremonyEvent::LateStepResultObserved(event) => started
            .claim_fence(record.ceremony_id())
            .is_ok_and(|fence| &fence == event.result.claim_fence()),
        _ => false,
    }
}
