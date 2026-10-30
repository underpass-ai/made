//! Keep long-running admissions live without allowing revoked work to claim again.
use std::sync::Arc;

use made_core::entities::{AuditFact, AuditRecord, ProtocolEvent};
use made_core::ports::ClockPort;
use made_core::value_objects::{
    AuthorizationAction, AuthorizationRequestId, AuthorizationTargetDigest, AuthorizedOperation,
    ProtocolId, StreamVersion,
};
use made_core::DomainError;

use super::{AuthorizeOperationUseCase, ContinueAcceptedProtocolWorkUseCase};
use crate::services::SessionStream;

pub struct AuthorizeProtocolAppendUseCase {
    authorize: Arc<AuthorizeOperationUseCase>,
    continuation: Arc<ContinueAcceptedProtocolWorkUseCase>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for AuthorizeProtocolAppendUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizeProtocolAppendUseCase")
            .finish_non_exhaustive()
    }
}

impl AuthorizeProtocolAppendUseCase {
    #[must_use]
    pub fn new(
        authorize: Arc<AuthorizeOperationUseCase>,
        continuation: Arc<ContinueAcceptedProtocolWorkUseCase>,
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
        protocol_id: &ProtocolId,
        version: StreamVersion,
        facts: &[AuditFact],
        operation: AuthorizedOperation,
    ) -> Result<AuthorizedOperation, DomainError> {
        let new_claim = operation.evidence().action() == AuthorizationAction::RunProtocol
            && facts
                .iter()
                .any(|fact| matches!(fact.event, ProtocolEvent::StepStarted(_)));
        if !new_claim && operation.evidence().is_live_at(self.clock.now()) {
            return Ok(operation);
        }
        let target = serde_json::to_vec(&(
            operation.evidence().decision_id(),
            protocol_id,
            version,
            facts
                .iter()
                .map(|fact| (&fact.event_id, &fact.event, &fact.actor, fact.occurred_at))
                .collect::<Vec<_>>(),
        ))
        .map_err(|_| DomainError::InvariantViolated {
            reason: "protocol append admission cannot be canonicalized",
        })?;
        let digest = AuthorizationTargetDigest::for_bytes(&target);
        let request_id = AuthorizationRequestId::new(format!("append-{}", digest.as_str()))?;
        if let Some(first) = facts.first() {
            if matches!(
                first.event,
                ProtocolEvent::StepCompleted(_)
                    | ProtocolEvent::StepFailed(_)
                    | ProtocolEvent::LateStepResultObserved(_)
            ) {
                let records = stream.records(protocol_id).await?;
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
                        AuthorizationAction::CompleteProtocolStep,
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
                reason: "current authorization refuses new protocol work",
            })?;
        AuthorizedOperation::new(operation.principal().clone(), evidence)
    }
}

fn claim_matches(record: &AuditRecord, result: &ProtocolEvent) -> bool {
    let Some(ProtocolEvent::StepStarted(started)) = record.event() else {
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
        ProtocolEvent::StepCompleted(event) => {
            coordinates
                == (
                    &event.step_id,
                    event.state_visit(),
                    event.state_iteration(),
                    event.iteration,
                    event.attempt,
                )
        }
        ProtocolEvent::StepFailed(event) => {
            coordinates
                == (
                    &event.step_id,
                    event.state_visit(),
                    event.state_iteration(),
                    event.iteration,
                    event.attempt,
                )
        }
        ProtocolEvent::LateStepResultObserved(event) => started
            .claim_fence(record.protocol_id())
            .is_ok_and(|fence| &fence == event.result.claim_fence()),
        _ => false,
    }
}
