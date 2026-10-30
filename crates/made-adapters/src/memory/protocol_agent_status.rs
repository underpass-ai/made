use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{
    AgentExecutionStatus, AgentLiveness, ProtocolAgentActivity, ProtocolAgentActivityKind,
    ProtocolAgentStatus,
};
use made_core::error::DomainError;
use made_core::ports::{
    ProtocolAgentActivityPage, ProtocolAgentActivityQuery, ProtocolAgentActivitySubscriptionPort,
    ProtocolAgentStatusPage, ProtocolAgentStatusPort, ProtocolAgentStatusQuery,
};
use made_core::value_objects::{AuthorizationAction, AuthorizationEvidence};
use tokio::sync::{watch, RwLock};

use super::protocol_agent_activity_subscription::ProtocolAgentActivitySubscription;

const ACTIVITY_RETENTION: usize = 1_000;

#[derive(Debug, Clone)]
pub struct InMemoryProtocolAgentStatus {
    entries: Arc<RwLock<BTreeMap<(String, String), ProtocolAgentStatus>>>,
    #[allow(clippy::type_complexity)] // receipt identity is scoped by protocol, execution and key
    receipts: Arc<RwLock<BTreeMap<(String, String, String), ProtocolAgentStatus>>>,
    #[allow(clippy::type_complexity)] // a retired concrete host incarnation must never return
    retired_incarnations: Arc<RwLock<BTreeSet<(String, String, String, String)>>>,
    activities: Arc<RwLock<BTreeMap<(String, u64), ProtocolAgentActivity>>>,
    activity_heads: Arc<RwLock<BTreeMap<String, u64>>>,
    activity_generation: watch::Sender<u64>,
}

impl InMemoryProtocolAgentStatus {
    #[must_use]
    pub fn new() -> Self {
        let (activity_generation, _) = watch::channel(0);
        Self {
            entries: Arc::default(),
            receipts: Arc::default(),
            retired_incarnations: Arc::default(),
            activities: Arc::default(),
            activity_heads: Arc::default(),
            activity_generation,
        }
    }
}

impl Default for InMemoryProtocolAgentStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolAgentStatusPort for InMemoryProtocolAgentStatus {
    fn subscribe_activity(&self) -> Box<dyn ProtocolAgentActivitySubscriptionPort> {
        Box::new(ProtocolAgentActivitySubscription::new(
            self.activity_generation.subscribe(),
        ))
    }

    async fn report(
        &self,
        status: ProtocolAgentStatus,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ProtocolAgentStatus, DomainError> {
        require_report_authorization(&status, authorization.as_ref())?;
        let receipt_key = (
            status.protocol_id().as_str().to_owned(),
            status.agent_execution_id().as_str().to_owned(),
            status.idempotency_key().to_owned(),
        );
        let mut receipts = self.receipts.write().await;
        if let Some(receipt) = receipts.get(&receipt_key) {
            return if receipt == &status {
                Ok(receipt.clone())
            } else {
                Err(DomainError::Conflict {
                    what: "protocol agent status idempotency key payload",
                })
            };
        }
        let mut entries = self.entries.write().await;
        let key = (
            status.protocol_id().as_str().to_owned(),
            status.agent_execution_id().as_str().to_owned(),
        );
        let previous = entries.get(&key).cloned();
        if let Some(previous) = previous.as_ref() {
            if !status.has_same_logical_execution_as(previous) {
                return Err(DomainError::Conflict {
                    what: "logical protocol agent execution identity",
                });
            }
            if status.report_sequence() <= previous.report_sequence() {
                return Err(DomainError::Conflict {
                    what: "out-of-order protocol agent status report",
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
                    what: "replaced protocol agent execution",
                });
            }
            if host_changed {
                self.retired_incarnations.write().await.insert((
                    previous.protocol_id().as_str().to_owned(),
                    previous.agent_execution_id().as_str().to_owned(),
                    previous.host_agent_id().as_str().to_owned(),
                    previous.host_agent_incarnation().as_str().to_owned(),
                ));
            }
        }
        if self.retired_incarnations.read().await.contains(&(
            status.protocol_id().as_str().to_owned(),
            status.agent_execution_id().as_str().to_owned(),
            status.host_agent_id().as_str().to_owned(),
            status.host_agent_incarnation().as_str().to_owned(),
        )) {
            return Err(DomainError::Conflict {
                what: "retired protocol agent incarnation",
            });
        }
        entries.insert(key, status.clone());
        receipts.insert(receipt_key, status.clone());
        self.append_activity(previous.as_ref(), status.clone())
            .await;
        Ok(status)
    }

    async fn get(
        &self,
        protocol_id: &str,
        agent_execution_id: &str,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ProtocolAgentStatus, DomainError> {
        // Two actions read one row, because delivering an intervention
        // has to ask who is holding this execution before it hands
        // anything over. Naming them both here keeps that check honest:
        // the pull is reading the roster, and the roster knows it.
        require_one_read_authorization(
            authorization.as_ref(),
            &[
                AuthorizationAction::GetProtocolAgent,
                AuthorizationAction::PullProtocolAgentInterventions,
            ],
        )?;
        self.entries
            .read()
            .await
            .get(&(protocol_id.to_owned(), agent_execution_id.to_owned()))
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "protocol agent execution",
            })
    }

    async fn list(
        &self,
        query: ProtocolAgentStatusQuery,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ProtocolAgentStatusPage, DomainError> {
        require_one_read_authorization(
            authorization.as_ref(),
            &[
                AuthorizationAction::ListProtocolAgents,
                // The projection that fills the delivery ledger reads
                // the roster to know which live agents a question put
                // to a seat should be offered to.
                AuthorizationAction::RequestProtocolIntervention,
            ],
        )?;
        let after = query.cursor().map(parse_cursor).transpose()?;
        let mut values: Vec<_> = self
            .entries
            .read()
            .await
            .values()
            .filter(|status| status.protocol_id().as_str() == query.protocol_id())
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
        Ok(ProtocolAgentStatusPage::new(entries, next_cursor))
    }

    async fn read_activity(
        &self,
        query: ProtocolAgentActivityQuery,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ProtocolAgentActivityPage, DomainError> {
        require_read_authorization(authorization.as_ref(), AuthorizationAction::StreamProtocol)?;
        // Keep the roster read guard while reading the activity boundary. A
        // report cannot become visible in only one half of this snapshot.
        let entries = self.entries.read().await;
        let activities = self.activities.read().await;
        let heads = self.activity_heads.read().await;
        let protocol_id = query.protocol_id().as_str();
        let head = heads.get(protocol_id).copied().unwrap_or(0);
        let oldest = activities
            .range((protocol_id.to_owned(), 0)..=(protocol_id.to_owned(), u64::MAX))
            .next()
            .map(|((_, sequence), _)| *sequence);
        let is_fresh_snapshot = query.after_sequence() == 0;
        if query.after_sequence() > head {
            return Err(DomainError::Conflict {
                what: "protocol agent activity cursor ahead of head",
            });
        }
        if oldest.is_some_and(|oldest| {
            query.after_sequence() > 0 && query.after_sequence().saturating_add(1) < oldest
        }) {
            return Err(DomainError::Conflict {
                what: "protocol agent activity cursor expired",
            });
        }
        let matching_snapshot = || {
            entries
                .values()
                .filter(|status| activity_matches(status, &query))
        };
        let snapshot_count = matching_snapshot().count();
        let snapshot = if is_fresh_snapshot {
            matching_snapshot().take(query.limit()).cloned().collect()
        } else {
            Vec::new()
        };
        // A fresh observer starts from an atomic roster at `head`; replaying
        // older activity beside that snapshot would duplicate already-folded
        // state. Reconnects instead resume strictly after their supplied
        // global cursor.
        let activity_after = if is_fresh_snapshot {
            head
        } else {
            query.after_sequence()
        };
        let mut scanned_through = activity_after;
        let mut selected = Vec::new();
        if activity_after < head {
            for ((_, sequence), activity) in activities.range(
                (protocol_id.to_owned(), activity_after.saturating_add(1))
                    ..=(protocol_id.to_owned(), head),
            ) {
                scanned_through = *sequence;
                if activity_matches(activity.status(), &query) {
                    selected.push(activity.clone());
                    if selected.len() == query.limit() {
                        break;
                    }
                }
            }
        }
        if selected.len() < query.limit() {
            // Advancing across filtered records is what keeps this a global
            // cursor instead of inventing a per-filter sequence.
            scanned_through = head;
        }
        Ok(ProtocolAgentActivityPage::new(
            snapshot,
            selected,
            scanned_through,
            head,
            oldest,
            snapshot_count <= query.limit(),
        ))
    }
}

impl InMemoryProtocolAgentStatus {
    async fn append_activity(
        &self,
        previous: Option<&ProtocolAgentStatus>,
        status: ProtocolAgentStatus,
    ) {
        let kind = activity_kind(previous, &status);
        if kind == ProtocolAgentActivityKind::Heartbeat
            && previous.is_some_and(|previous| inert_heartbeat(previous, &status))
        {
            return;
        }
        let protocol_id = status.protocol_id().as_str().to_owned();
        let mut heads = self.activity_heads.write().await;
        let head = heads.entry(protocol_id.clone()).or_default();
        *head = head.saturating_add(1);
        let sequence = *head;
        let mut activities = self.activities.write().await;
        activities.insert(
            (protocol_id.clone(), sequence),
            ProtocolAgentActivity::new(sequence, kind, status),
        );
        while activities
            .range((protocol_id.clone(), 0)..=(protocol_id.clone(), u64::MAX))
            .count()
            > ACTIVITY_RETENTION
        {
            let Some(key) = activities
                .range((protocol_id.clone(), 0)..=(protocol_id.clone(), u64::MAX))
                .next()
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            activities.remove(&key);
        }
        self.activity_generation.send_modify(|generation| {
            *generation = generation.wrapping_add(1);
        });
    }
}

fn activity_matches(status: &ProtocolAgentStatus, query: &ProtocolAgentActivityQuery) -> bool {
    status.protocol_id() == query.protocol_id()
        && query.role_id().is_none_or(|role| status.role_id() == role)
        && query.step_id().is_none_or(|step| status.step_id() == step)
        && query
            .agent_execution_id()
            .is_none_or(|agent| status.agent_execution_id() == agent)
}

fn activity_kind(
    previous: Option<&ProtocolAgentStatus>,
    status: &ProtocolAgentStatus,
) -> ProtocolAgentActivityKind {
    use ProtocolAgentActivityKind as K;
    match status.activity() {
        "checkpoint_available" => K::CheckpointAvailable,
        "intervention_delivered" => K::InterventionDelivered,
        "intervention_answered" => K::InterventionAnswered,
        "input_requested" => K::InputRequested,
        "failed" => K::Failed,
        "heartbeat" => K::Heartbeat,
        _ if previous.is_some_and(|previous| {
            previous.host_agent_id() != status.host_agent_id()
                || previous.host_agent_incarnation() != status.host_agent_incarnation()
        }) =>
        {
            K::AgentReplaced
        }
        _ if status.execution_status() == AgentExecutionStatus::Finished => K::Completed,
        _ if status.liveness() == AgentLiveness::Stale => K::Stale,
        _ if status.blocker().is_some()
            && previous.is_none_or(|previous| previous.blocker() != status.blocker()) =>
        {
            K::BlockerOpened
        }
        _ if status.blocker().is_none()
            && previous.is_some_and(|previous| previous.blocker().is_some()) =>
        {
            K::BlockerResolved
        }
        _ => K::ActivityChanged,
    }
}

fn inert_heartbeat(previous: &ProtocolAgentStatus, status: &ProtocolAgentStatus) -> bool {
    previous.activity() == "heartbeat"
        && previous.execution_status() == status.execution_status()
        && previous.liveness() == status.liveness()
        && previous.blocker() == status.blocker()
        && previous.dependency() == status.dependency()
        && previous.task_summary() == status.task_summary()
        && previous.evidence_references() == status.evidence_references()
        && previous.usage_kind() == status.usage_kind()
        && previous.usage_value() == status.usage_value()
}

fn require_report_authorization(
    status: &ProtocolAgentStatus,
    authorization: Option<&AuthorizationEvidence>,
) -> Result<(), DomainError> {
    let authorization = authorization.ok_or(DomainError::InvariantViolated {
        reason: "protocol agent status storage requires authorization evidence",
    })?;
    if authorization.action() != AuthorizationAction::ReportProtocolAgentStatus
        || authorization.principal_id().as_str() != status.claim_owner_id().as_str()
        || status.host_agent_id() != status.claim_owner_id()
    {
        return Err(DomainError::InvariantViolated {
            reason: "protocol agent status authorization does not own the reported claim",
        });
    }
    Ok(())
}

fn require_read_authorization(
    authorization: Option<&AuthorizationEvidence>,
    expected_action: AuthorizationAction,
) -> Result<(), DomainError> {
    require_one_read_authorization(authorization, &[expected_action])
}

fn require_one_read_authorization(
    authorization: Option<&AuthorizationEvidence>,
    expected: &[AuthorizationAction],
) -> Result<(), DomainError> {
    let authorization = authorization.ok_or(DomainError::InvariantViolated {
        reason: "protocol agent status reads require authorization evidence",
    })?;
    if !expected.contains(&authorization.action()) {
        return Err(DomainError::InvariantViolated {
            reason: "protocol agent status authorization action does not match the read",
        });
    }
    Ok(())
}

fn parse_cursor(cursor: &str) -> Result<String, DomainError> {
    cursor
        .strip_prefix("protocol-agent:v1:")
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or(DomainError::InvariantViolated {
            reason: "agent status cursor is not a valid keyset cursor",
        })
}

fn cursor_for(status: &ProtocolAgentStatus) -> String {
    format!("protocol-agent:v1:{}", status.agent_execution_id().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::entities::{AgentExecutionStatus, AgentLiveness, AgentStatusSource};
    use made_core::value_objects::{
        ExecutionOperationId, HostAgentIncarnation, LeaseOwnerId, LogicalWorkerId,
        ProtocolAgentExecutionId, ProtocolId, RoleId, StepClaimFence, StepId,
    };
    use serde_json::json;
    use time::OffsetDateTime;

    fn authorization() -> AuthorizationEvidence {
        authorization_for("report_protocol_agent_status", "host")
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
    ) -> ProtocolAgentStatus {
        ProtocolAgentStatus::new(
            ProtocolId::new("c").unwrap(),
            ProtocolAgentExecutionId::new(execution).unwrap(),
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

    fn status(sequence: u64, key: &str, incarnation: &str) -> ProtocolAgentStatus {
        status_for(
            "e",
            sequence,
            key,
            incarnation,
            AgentExecutionStatus::Running,
        )
    }

    fn progress_status(
        execution: &str,
        role_id: &str,
        step_id: &str,
        sequence: u64,
        key: &str,
        activity: &str,
        blocker: Option<&str>,
    ) -> ProtocolAgentStatus {
        ProtocolAgentStatus::new(
            ProtocolId::new("c").unwrap(),
            ProtocolAgentExecutionId::new(execution).unwrap(),
            ExecutionOperationId::new("1".repeat(64)).unwrap(),
            LeaseOwnerId::new("host").unwrap(),
            LogicalWorkerId::new(format!("worker-{execution}")).unwrap(),
            LeaseOwnerId::new("host").unwrap(),
            HostAgentIncarnation::new("inc-1").unwrap(),
            None,
            None,
            RoleId::new(role_id).unwrap(),
            StepId::new(step_id).unwrap(),
            1,
            AgentExecutionStatus::Running,
            AgentLiveness::Fresh,
            AgentStatusSource::HostReport,
            None,
            None,
            None,
            None,
            activity,
            blocker.map(str::to_owned),
            None,
            "bounded",
            vec![format!("artifact:{execution}")],
            None,
            None,
            OffsetDateTime::UNIX_EPOCH,
            sequence,
            key,
            StepClaimFence::new("2".repeat(64)).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn reports_are_idempotent_and_old_incarnations_are_fenced() {
        let store = InMemoryProtocolAgentStatus::new();
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
        let replacement = ProtocolAgentStatus::new(
            ProtocolId::new("c").unwrap(),
            ProtocolAgentExecutionId::new("e").unwrap(),
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
        let store = InMemoryProtocolAgentStatus::new();
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
        let store = InMemoryProtocolAgentStatus::new();
        let initial = status(1, "invented", "inc-1");
        assert!(store.report(initial.clone(), None).await.is_err());
        let foreign = authorization_for("report_protocol_agent_status", "other-host");
        assert!(store.report(initial, Some(foreign)).await.is_err());
        assert!(store
            .get(
                "c",
                "e",
                Some(authorization_for("get_protocol_agent", "host"))
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn list_uses_stable_keyset_pages_and_keeps_execution_states_distinct() {
        let store = InMemoryProtocolAgentStatus::new();
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
                ProtocolAgentStatusQuery::new("c", None, 2, None).unwrap(),
                Some(authorization_for("list_protocol_agents", "host")),
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
                ProtocolAgentStatusQuery::new("c", first.next_cursor().map(str::to_owned), 2, None)
                    .unwrap(),
                Some(authorization_for("list_protocol_agents", "host")),
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
                ProtocolAgentStatusQuery::new("c", Some("0".into()), 2, None).unwrap(),
                None,
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn activity_filters_keep_the_global_cursor_and_atomic_snapshot() {
        let store = InMemoryProtocolAgentStatus::new();
        for (execution, role, step) in [
            ("agent-a", "author", "draft"),
            ("agent-b", "reviewer", "review"),
            ("agent-c", "integrator", "merge"),
        ] {
            store
                .report(
                    progress_status(execution, role, step, 1, execution, "editing", None),
                    Some(authorization()),
                )
                .await
                .unwrap();
        }
        let query = ProtocolAgentActivityQuery::new(
            ProtocolId::new("c").unwrap(),
            0,
            10,
            Some(RoleId::new("reviewer").unwrap()),
            None,
            None,
        )
        .unwrap();
        let page = store
            .read_activity(
                query,
                Some(authorization_for("stream_protocol", "observer")),
            )
            .await
            .unwrap();
        assert_eq!(page.snapshot().len(), 1);
        assert!(page.activities().is_empty());
        assert_eq!(page.next_sequence(), 3);
        assert_eq!(page.head_sequence(), 3);

        store
            .report(
                progress_status(
                    "agent-a",
                    "author",
                    "draft",
                    2,
                    "agent-a-2",
                    "testing",
                    None,
                ),
                Some(authorization()),
            )
            .await
            .unwrap();
        let resumed = store
            .read_activity(
                ProtocolAgentActivityQuery::new(
                    ProtocolId::new("c").unwrap(),
                    page.next_sequence(),
                    10,
                    Some(RoleId::new("reviewer").unwrap()),
                    None,
                    None,
                )
                .unwrap(),
                Some(authorization_for("stream_protocol", "observer")),
            )
            .await
            .unwrap();
        assert!(resumed.activities().is_empty());
        assert_eq!(resumed.next_sequence(), 4);

        store
            .report(
                progress_status(
                    "agent-b",
                    "reviewer",
                    "review",
                    2,
                    "agent-b-2",
                    "testing",
                    None,
                ),
                Some(authorization()),
            )
            .await
            .unwrap();
        let matching = store
            .read_activity(
                ProtocolAgentActivityQuery::new(
                    ProtocolId::new("c").unwrap(),
                    resumed.next_sequence(),
                    10,
                    Some(RoleId::new("reviewer").unwrap()),
                    None,
                    None,
                )
                .unwrap(),
                Some(authorization_for("stream_protocol", "observer")),
            )
            .await
            .unwrap();
        assert_eq!(matching.activities()[0].sequence(), 5);
    }

    #[tokio::test]
    async fn meaningful_blockers_survive_while_inert_heartbeats_are_coalesced() {
        let store = InMemoryProtocolAgentStatus::new();
        for report in [
            progress_status("agent", "role", "step", 1, "baseline", "editing", None),
            progress_status("agent", "role", "step", 2, "open", "editing", Some("lock")),
            progress_status("agent", "role", "step", 3, "resolved", "editing", None),
            progress_status("agent", "role", "step", 4, "heartbeat-1", "heartbeat", None),
            progress_status("agent", "role", "step", 5, "heartbeat-2", "heartbeat", None),
        ] {
            store.report(report, Some(authorization())).await.unwrap();
        }
        let page = store
            .read_activity(
                ProtocolAgentActivityQuery::new(
                    ProtocolId::new("c").unwrap(),
                    1,
                    10,
                    None,
                    None,
                    None,
                )
                .unwrap(),
                Some(authorization_for("stream_protocol", "observer")),
            )
            .await
            .unwrap();
        assert_eq!(
            page.activities()
                .iter()
                .map(ProtocolAgentActivity::kind)
                .collect::<Vec<_>>(),
            vec![
                ProtocolAgentActivityKind::BlockerOpened,
                ProtocolAgentActivityKind::BlockerResolved,
                ProtocolAgentActivityKind::Heartbeat,
            ]
        );
        assert_eq!(page.head_sequence(), 4);
    }

    #[tokio::test]
    async fn a_report_racing_the_fresh_snapshot_is_never_lost() {
        let store = InMemoryProtocolAgentStatus::new();
        store
            .report(
                progress_status("agent", "role", "step", 1, "baseline", "editing", None),
                Some(authorization()),
            )
            .await
            .unwrap();

        let snapshot_store = store.clone();
        let report_store = store.clone();
        let (snapshot, ()) = tokio::join!(
            async move {
                snapshot_store
                    .read_activity(
                        ProtocolAgentActivityQuery::new(
                            ProtocolId::new("c").unwrap(),
                            0,
                            10,
                            None,
                            None,
                            None,
                        )
                        .unwrap(),
                        Some(authorization_for("stream_protocol", "observer")),
                    )
                    .await
                    .unwrap()
            },
            async move {
                report_store
                    .report(
                        progress_status("agent", "role", "step", 2, "racing", "testing", None),
                        Some(authorization()),
                    )
                    .await
                    .unwrap();
            }
        );
        assert!(snapshot.activities().is_empty());
        let snapshot_report = snapshot.snapshot()[0].report_sequence();
        let resumed = store
            .read_activity(
                ProtocolAgentActivityQuery::new(
                    ProtocolId::new("c").unwrap(),
                    snapshot.next_sequence(),
                    10,
                    None,
                    None,
                    None,
                )
                .unwrap(),
                Some(authorization_for("stream_protocol", "observer")),
            )
            .await
            .unwrap();
        let recovered_report = resumed
            .activities()
            .last()
            .map_or(snapshot_report, |activity| {
                activity.status().report_sequence()
            });
        assert_eq!(recovered_report, 2);
    }

    #[tokio::test]
    async fn activity_read_requires_stream_authorization() {
        let store = InMemoryProtocolAgentStatus::new();
        let query =
            ProtocolAgentActivityQuery::new(ProtocolId::new("c").unwrap(), 0, 10, None, None, None)
                .unwrap();
        assert!(store.read_activity(query.clone(), None).await.is_err());
        assert!(store
            .read_activity(
                query,
                Some(authorization_for("get_protocol_agent", "observer"))
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn expired_activity_cursors_fail_instead_of_hiding_retention_gaps() {
        let store = InMemoryProtocolAgentStatus::new();
        for index in 0..(ACTIVITY_RETENTION + 2) {
            let execution = format!("agent-{index:04}");
            store
                .report(
                    progress_status(
                        &execution,
                        "role",
                        "step",
                        1,
                        &format!("report-{index}"),
                        "editing",
                        None,
                    ),
                    Some(authorization()),
                )
                .await
                .unwrap();
        }
        let expired = store
            .read_activity(
                ProtocolAgentActivityQuery::new(
                    ProtocolId::new("c").unwrap(),
                    1,
                    10,
                    None,
                    None,
                    None,
                )
                .unwrap(),
                Some(authorization_for("stream_protocol", "observer")),
            )
            .await
            .unwrap_err();
        assert_eq!(
            expired,
            DomainError::Conflict {
                what: "protocol agent activity cursor expired"
            }
        );
    }

    #[tokio::test]
    async fn a_cursor_from_a_prior_store_incarnation_is_rejected_after_restart() {
        let restarted_store = InMemoryProtocolAgentStatus::new();
        let error = restarted_store
            .read_activity(
                ProtocolAgentActivityQuery::new(
                    ProtocolId::new("c").unwrap(),
                    8,
                    10,
                    None,
                    None,
                    None,
                )
                .unwrap(),
                Some(authorization_for("stream_protocol", "observer")),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error,
            DomainError::Conflict {
                what: "protocol agent activity cursor ahead of head"
            }
        );
    }
}
