use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::CeremonyAgentStatus;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyAgentStatusPage, CeremonyAgentStatusPort, CeremonyAgentStatusQuery,
};
use made_core::value_objects::{AuthorizationAction, AuthorizationEvidence};
use tokio::sync::RwLock;

#[derive(Debug, Default, Clone)]
pub struct InMemoryCeremonyAgentStatus {
    entries: Arc<RwLock<BTreeMap<(String, String), CeremonyAgentStatus>>>,
    #[allow(clippy::type_complexity)] // receipt identity is scoped by ceremony, execution and key
    receipts: Arc<RwLock<BTreeMap<(String, String, String), CeremonyAgentStatus>>>,
    #[allow(clippy::type_complexity)] // a retired concrete host incarnation must never return
    retired_incarnations: Arc<RwLock<BTreeSet<(String, String, String, String)>>>,
}

impl InMemoryCeremonyAgentStatus {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl CeremonyAgentStatusPort for InMemoryCeremonyAgentStatus {
    async fn report(
        &self,
        status: CeremonyAgentStatus,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<CeremonyAgentStatus, DomainError> {
        require_report_authorization(&status, authorization.as_ref())?;
        let receipt_key = (
            status.ceremony_id().as_str().to_owned(),
            status.agent_execution_id().as_str().to_owned(),
            status.idempotency_key().to_owned(),
        );
        let mut receipts = self.receipts.write().await;
        if let Some(receipt) = receipts.get(&receipt_key) {
            return if receipt == &status {
                Ok(receipt.clone())
            } else {
                Err(DomainError::Conflict {
                    what: "ceremony agent status idempotency key payload",
                })
            };
        }
        let mut entries = self.entries.write().await;
        let key = (
            status.ceremony_id().as_str().to_owned(),
            status.agent_execution_id().as_str().to_owned(),
        );
        if let Some(previous) = entries.get(&key) {
            if !status.has_same_logical_execution_as(previous) {
                return Err(DomainError::Conflict {
                    what: "logical ceremony agent execution identity",
                });
            }
            if status.report_sequence() <= previous.report_sequence() {
                return Err(DomainError::Conflict {
                    what: "out-of-order ceremony agent status report",
                });
            }
            let host_changed = previous.host_agent_id() != status.host_agent_id()
                || previous.host_agent_incarnation() != status.host_agent_incarnation();
            if host_changed
                && (status.previous_host_agent_id() != Some(previous.host_agent_id())
                    || status.previous_host_agent_incarnation()
                        != Some(previous.host_agent_incarnation()))
            {
                return Err(DomainError::Conflict {
                    what: "replaced ceremony agent execution",
                });
            }
            if host_changed {
                self.retired_incarnations.write().await.insert((
                    previous.ceremony_id().as_str().to_owned(),
                    previous.agent_execution_id().as_str().to_owned(),
                    previous.host_agent_id().as_str().to_owned(),
                    previous.host_agent_incarnation().as_str().to_owned(),
                ));
            }
        }
        if self.retired_incarnations.read().await.contains(&(
            status.ceremony_id().as_str().to_owned(),
            status.agent_execution_id().as_str().to_owned(),
            status.host_agent_id().as_str().to_owned(),
            status.host_agent_incarnation().as_str().to_owned(),
        )) {
            return Err(DomainError::Conflict {
                what: "retired ceremony agent incarnation",
            });
        }
        entries.insert(key, status.clone());
        receipts.insert(receipt_key, status.clone());
        Ok(status)
    }

    async fn get(
        &self,
        ceremony_id: &str,
        agent_execution_id: &str,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<CeremonyAgentStatus, DomainError> {
        require_read_authorization(
            authorization.as_ref(),
            AuthorizationAction::GetCeremonyAgent,
        )?;
        self.entries
            .read()
            .await
            .get(&(ceremony_id.to_owned(), agent_execution_id.to_owned()))
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "ceremony agent execution",
            })
    }

    async fn list(
        &self,
        query: CeremonyAgentStatusQuery,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<CeremonyAgentStatusPage, DomainError> {
        require_read_authorization(
            authorization.as_ref(),
            AuthorizationAction::ListCeremonyAgents,
        )?;
        let after = query.cursor().map(parse_cursor).transpose()?;
        let mut values: Vec<_> = self
            .entries
            .read()
            .await
            .values()
            .filter(|status| status.ceremony_id().as_str() == query.ceremony_id())
            .filter(|status| {
                query
                    .execution_status()
                    .is_none_or(|expected| status.execution_status() == expected)
            })
            .cloned()
            .collect();
        values.sort_by(|left, right| {
            left.agent_execution_id()
                .cmp(right.agent_execution_id())
                .then(left.report_sequence().cmp(&right.report_sequence()))
        });
        if let Some(after) = after.as_deref() {
            values.retain(|status| status.agent_execution_id().as_str() > after);
        }
        let has_more = values.len() > query.limit();
        let entries: Vec<_> = values.into_iter().take(query.limit()).collect();
        let next_cursor = has_more.then(|| cursor_for(entries.last().expect("non-empty page")));
        Ok(CeremonyAgentStatusPage::new(entries, next_cursor))
    }
}

fn require_report_authorization(
    status: &CeremonyAgentStatus,
    authorization: Option<&AuthorizationEvidence>,
) -> Result<(), DomainError> {
    let authorization = authorization.ok_or(DomainError::InvariantViolated {
        reason: "ceremony agent status storage requires authorization evidence",
    })?;
    if authorization.action() != AuthorizationAction::ReportCeremonyAgentStatus
        || authorization.principal_id().as_str() != status.claim_owner_id().as_str()
        || status.host_agent_id() != status.claim_owner_id()
    {
        return Err(DomainError::InvariantViolated {
            reason: "ceremony agent status authorization does not own the reported claim",
        });
    }
    Ok(())
}

fn require_read_authorization(
    authorization: Option<&AuthorizationEvidence>,
    expected_action: AuthorizationAction,
) -> Result<(), DomainError> {
    let authorization = authorization.ok_or(DomainError::InvariantViolated {
        reason: "ceremony agent status reads require authorization evidence",
    })?;
    if authorization.action() != expected_action {
        return Err(DomainError::InvariantViolated {
            reason: "ceremony agent status authorization action does not match the read",
        });
    }
    Ok(())
}

fn parse_cursor(cursor: &str) -> Result<String, DomainError> {
    cursor
        .strip_prefix("ceremony-agent:v1:")
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or(DomainError::InvariantViolated {
            reason: "agent status cursor is not a valid keyset cursor",
        })
}

fn cursor_for(status: &CeremonyAgentStatus) -> String {
    format!("ceremony-agent:v1:{}", status.agent_execution_id().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::entities::{AgentExecutionStatus, AgentLiveness, AgentStatusSource};
    use made_core::value_objects::{
        CeremonyAgentExecutionId, CeremonyId, ExecutionOperationId, HostAgentIncarnation,
        LeaseOwnerId, LogicalWorkerId, RoleId, StepClaimFence, StepId,
    };
    use serde_json::json;
    use time::OffsetDateTime;

    fn authorization() -> AuthorizationEvidence {
        authorization_for("report_ceremony_agent_status", "host")
    }

    fn authorization_for(action: &str, principal_id: &str) -> AuthorizationEvidence {
        serde_json::from_value(json!({
            "decision_id": "a".repeat(64),
            "request_id": "agent-status-report",
            "principal_id": principal_id,
            "action": action,
            "scope": {"kind": "global"},
            "target_digest": "b".repeat(64),
            "policy_version": 1,
            "admitted_at": "2026-09-19T12:00:00Z",
            "valid_until": "2026-09-19T12:01:00Z"
        }))
        .unwrap()
    }

    fn status_for(
        execution: &str,
        sequence: u64,
        key: &str,
        incarnation: &str,
        execution_status: AgentExecutionStatus,
    ) -> CeremonyAgentStatus {
        CeremonyAgentStatus::new(
            CeremonyId::new("c").unwrap(),
            CeremonyAgentExecutionId::new(execution).unwrap(),
            ExecutionOperationId::new("1".repeat(64)).unwrap(),
            LeaseOwnerId::new("host").unwrap(),
            LogicalWorkerId::new("worker").unwrap(),
            LeaseOwnerId::new("host").unwrap(),
            HostAgentIncarnation::new(incarnation).unwrap(),
            None,
            None,
            RoleId::new("role").unwrap(),
            StepId::new("step").unwrap(),
            1,
            execution_status,
            AgentLiveness::Fresh,
            AgentStatusSource::HostReport,
            None,
            None,
            None,
            None,
            "editing",
            None,
            None,
            "bounded",
            vec![],
            None,
            None,
            OffsetDateTime::UNIX_EPOCH,
            sequence,
            key,
            StepClaimFence::new("2".repeat(64)).unwrap(),
        )
        .unwrap()
    }

    fn status(sequence: u64, key: &str, incarnation: &str) -> CeremonyAgentStatus {
        status_for(
            "e",
            sequence,
            key,
            incarnation,
            AgentExecutionStatus::Running,
        )
    }

    #[tokio::test]
    async fn reports_are_idempotent_and_old_incarnations_are_fenced() {
        let store = InMemoryCeremonyAgentStatus::new();
        let first = store
            .report(status(1, "k", "inc-1"), Some(authorization()))
            .await
            .unwrap();
        assert_eq!(
            store
                .report(status(1, "k", "inc-1"), Some(authorization()))
                .await
                .unwrap(),
            first
        );
        assert!(store
            .report(status(1, "other", "inc-1"), Some(authorization()))
            .await
            .is_err());
        let replacement = CeremonyAgentStatus::new(
            CeremonyId::new("c").unwrap(),
            CeremonyAgentExecutionId::new("e").unwrap(),
            ExecutionOperationId::new("1".repeat(64)).unwrap(),
            LeaseOwnerId::new("host").unwrap(),
            LogicalWorkerId::new("worker").unwrap(),
            LeaseOwnerId::new("host").unwrap(),
            HostAgentIncarnation::new("inc-2").unwrap(),
            Some(LeaseOwnerId::new("host").unwrap()),
            Some(HostAgentIncarnation::new("inc-1").unwrap()),
            RoleId::new("role").unwrap(),
            StepId::new("step").unwrap(),
            1,
            AgentExecutionStatus::Running,
            AgentLiveness::Fresh,
            AgentStatusSource::HostReport,
            None,
            None,
            None,
            None,
            "editing",
            None,
            None,
            "bounded",
            vec![],
            None,
            None,
            OffsetDateTime::UNIX_EPOCH,
            2,
            "replacement",
            StepClaimFence::new("2".repeat(64)).unwrap(),
        )
        .unwrap();
        assert!(store
            .report(replacement, Some(authorization()))
            .await
            .is_ok());
        assert!(store
            .report(status(3, "late-old", "inc-1"), Some(authorization()))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn report_replay_is_exact_and_conflicting_payload_is_rejected() {
        let store = InMemoryCeremonyAgentStatus::new();
        let first = status(1, "replay", "inc-1");
        assert_eq!(
            store
                .report(first.clone(), Some(authorization()))
                .await
                .unwrap(),
            first
        );
        assert_eq!(
            store
                .report(first.clone(), Some(authorization()))
                .await
                .unwrap(),
            first
        );
        let conflict = status_for("e", 1, "replay", "inc-1", AgentExecutionStatus::Waiting);
        assert!(store.report(conflict, Some(authorization())).await.is_err());
        assert!(store
            .report(status(1, "old", "inc-1"), Some(authorization()))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn rejects_an_invented_or_foreign_initial_report_before_persisting() {
        let store = InMemoryCeremonyAgentStatus::new();
        let initial = status(1, "invented", "inc-1");
        assert!(store.report(initial.clone(), None).await.is_err());
        let foreign = authorization_for("report_ceremony_agent_status", "other-host");
        assert!(store.report(initial, Some(foreign)).await.is_err());
        assert!(store
            .get(
                "c",
                "e",
                Some(authorization_for("get_ceremony_agent", "host"))
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn list_uses_stable_keyset_pages_and_keeps_execution_states_distinct() {
        let store = InMemoryCeremonyAgentStatus::new();
        for (execution, state) in [
            ("execution-a", AgentExecutionStatus::Running),
            ("execution-b", AgentExecutionStatus::Waiting),
            ("execution-c", AgentExecutionStatus::Blocked),
            ("execution-d", AgentExecutionStatus::Finished),
        ] {
            store
                .report(
                    status_for(execution, 1, execution, "inc-1", state),
                    Some(authorization()),
                )
                .await
                .unwrap();
        }
        let first = store
            .list(
                CeremonyAgentStatusQuery::new("c", None, 2, None).unwrap(),
                Some(authorization_for("list_ceremony_agents", "host")),
            )
            .await
            .unwrap();
        assert_eq!(first.entries().len(), 2);
        assert_eq!(
            first.entries()[0].execution_status(),
            AgentExecutionStatus::Running
        );
        assert_eq!(
            first.entries()[1].execution_status(),
            AgentExecutionStatus::Waiting
        );
        // A newly inserted item before the keyset cursor cannot shift or
        // duplicate the next page.
        store
            .report(
                status_for(
                    "execution-0",
                    1,
                    "zero",
                    "inc-1",
                    AgentExecutionStatus::Running,
                ),
                Some(authorization()),
            )
            .await
            .unwrap();
        let second = store
            .list(
                CeremonyAgentStatusQuery::new("c", first.next_cursor().map(str::to_owned), 2, None)
                    .unwrap(),
                Some(authorization_for("list_ceremony_agents", "host")),
            )
            .await
            .unwrap();
        assert_eq!(
            second
                .entries()
                .iter()
                .map(|status| status.agent_execution_id().as_str())
                .collect::<Vec<_>>(),
            vec!["execution-c", "execution-d"]
        );
        assert!(store
            .list(
                CeremonyAgentStatusQuery::new("c", Some("0".into()), 2, None).unwrap(),
                None,
            )
            .await
            .is_err());
    }
}
