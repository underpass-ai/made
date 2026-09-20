use std::sync::Arc;

use made_core::entities::CeremonyAgentStatus;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyAgentStatusPage, CeremonyAgentStatusPort, CeremonyAgentStatusQuery, ClockPort,
};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationEvidence, ExecutionOperationId, StepStatus,
};
use time::Duration;

use crate::services::{AuthorizationOperationScope, SessionStream};

/// Application boundary for the live-agent status verbs.
pub struct CeremonyAgentStatusService {
    port: Arc<dyn CeremonyAgentStatusPort>,
    clock: Arc<dyn ClockPort>,
    stale_after: Duration,
    journal: Option<Arc<SessionStream>>,
}

impl std::fmt::Debug for CeremonyAgentStatusService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyAgentStatusService")
            .finish_non_exhaustive()
    }
}

impl CeremonyAgentStatusService {
    #[must_use]
    pub fn new(
        port: Arc<dyn CeremonyAgentStatusPort>,
        clock: Arc<dyn ClockPort>,
        stale_after: Duration,
    ) -> Self {
        Self {
            port,
            clock,
            stale_after,
            journal: None,
        }
    }

    #[must_use]
    pub fn with_journal_claims(mut self, journal: Arc<SessionStream>) -> Self {
        self.journal = Some(journal);
        self
    }

    pub async fn report(
        &self,
        status: CeremonyAgentStatus,
    ) -> Result<CeremonyAgentStatus, DomainError> {
        let authorization = current_authorization().ok_or(DomainError::InvariantViolated {
            reason: "ceremony agent status reports require authorization evidence",
        })?;
        self.verify_current_claim(&status, &authorization).await?;
        self.port.report(status, Some(authorization)).await
    }

    pub async fn get(
        &self,
        ceremony_id: &str,
        agent_execution_id: &str,
    ) -> Result<CeremonyAgentStatus, DomainError> {
        self.port
            .get(ceremony_id, agent_execution_id, current_authorization())
            .await
            .map(|status| status.projected_at(self.clock.now(), self.stale_after))
    }

    pub async fn list(
        &self,
        query: CeremonyAgentStatusQuery,
    ) -> Result<CeremonyAgentStatusPage, DomainError> {
        self.port
            .list(query, current_authorization())
            .await
            .map(|page| {
                CeremonyAgentStatusPage::new(
                    page.entries()
                        .iter()
                        .map(|status| status.projected_at(self.clock.now(), self.stale_after))
                        .collect(),
                    page.next_cursor().map(str::to_owned),
                )
            })
    }

    async fn verify_current_claim(
        &self,
        status: &CeremonyAgentStatus,
        authorization: &AuthorizationEvidence,
    ) -> Result<(), DomainError> {
        if authorization.action() != AuthorizationAction::ReportCeremonyAgentStatus {
            return Err(DomainError::InvariantViolated {
                reason: "agent status authorization action is not report_ceremony_agent_status",
            });
        }
        if authorization.principal_id().as_str() != status.claim_owner_id().as_str()
            || status.host_agent_id() != status.claim_owner_id()
        {
            return Err(DomainError::InvariantViolated {
                reason: "agent status reporter is not the current claim owner",
            });
        }
        let journal = self
            .journal
            .as_ref()
            .ok_or(DomainError::InvariantViolated {
                reason: "ceremony agent status claim verifier is not wired",
            })?;
        let ceremony_id = status.ceremony_id();
        let step_id = status.step_id();
        let session = journal.load(ceremony_id).await?;
        let record = session
            .instance
            .step_record(step_id)
            .ok_or(DomainError::NotFound {
                what: "current ceremony step claim",
            })?;
        let lease = record.lease().ok_or(DomainError::InvariantViolated {
            reason: "ceremony agent status step has no accepted lease",
        })?;
        let operation_id = ExecutionOperationId::for_step(
            ceremony_id,
            step_id,
            record.state_visit(),
            record.state_iteration(),
            record.iteration(),
        );
        require_live_status_claim(record, self.clock.now())?;
        if session.instance.step_claim_fence(step_id)? != *status.claim_fence()
            || lease.owner_id() != status.claim_owner_id()
            || record.attempt().get() != status.attempt()
            || operation_id != *status.operation_id()
        {
            return Err(DomainError::Conflict {
                what: "current ceremony agent status claim",
            });
        }
        Ok(())
    }
}

fn require_live_status_claim(
    record: &made_core::value_objects::StepExecutionRecord,
    now: time::OffsetDateTime,
) -> Result<(), DomainError> {
    if record.status() != StepStatus::InProgress || !record.has_live_lease_at(now) {
        return Err(DomainError::Conflict {
            what: "live ceremony agent status claim",
        });
    }
    Ok(())
}

fn current_authorization() -> Option<AuthorizationEvidence> {
    AuthorizationOperationScope::current().map(|operation| operation.evidence().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{
        IdempotencyKey, LeaseOwnerId, StepAttempt, StepErrorMessage, StepExecutionRecord,
        StepLease, StepOutput, StepResult,
    };
    use time::{Duration, OffsetDateTime};

    fn claimed(expires_at: OffsetDateTime) -> StepExecutionRecord {
        StepExecutionRecord::pending().with_started(
            StepLease::new(
                LeaseOwnerId::new("host").unwrap(),
                IdempotencyKey::new("status-claim").unwrap(),
                expires_at - Duration::seconds(60),
                expires_at,
            )
            .unwrap(),
            StepAttempt::FIRST,
            None,
        )
    }

    #[test]
    fn completed_or_failed_claim_cannot_accept_a_status_report() {
        let at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(30);
        let completed = claimed(at + Duration::seconds(30))
            .with_result(StepResult::completed(StepOutput::empty()).unwrap());
        let failed = claimed(at + Duration::seconds(30))
            .with_result(StepResult::failed(StepErrorMessage::new("failed").unwrap()).unwrap());
        assert!(require_live_status_claim(&completed, at).is_err());
        assert!(require_live_status_claim(&failed, at).is_err());
    }

    #[test]
    fn expired_claim_cannot_accept_a_status_report() {
        let expires_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(30);
        assert!(require_live_status_claim(&claimed(expires_at), expires_at).is_err());
    }
}
