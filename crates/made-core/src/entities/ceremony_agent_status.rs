use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use super::{AgentExecutionStatus, AgentLiveness, AgentStatusSource, AgentUsageKind};
use crate::error::DomainError;
use crate::value_objects::{
    CeremonyAgentExecutionId, CeremonyId, ExecutionOperationId, HostAgentIncarnation, LeaseOwnerId,
    LogicalWorkerId, RoleId, StepClaimFence, StepId,
};

const MAX_STATUS_TEXT: usize = 512;
const MAX_EVIDENCE_REFERENCES: usize = 20;

/// Bounded host-owned status evidence for one logical worker execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyAgentStatus {
    ceremony_id: CeremonyId,
    agent_execution_id: CeremonyAgentExecutionId,
    operation_id: ExecutionOperationId,
    claim_owner_id: LeaseOwnerId,
    logical_worker_id: LogicalWorkerId,
    host_agent_id: LeaseOwnerId,
    host_agent_incarnation: HostAgentIncarnation,
    previous_host_agent_id: Option<LeaseOwnerId>,
    previous_host_agent_incarnation: Option<HostAgentIncarnation>,
    role_id: RoleId,
    step_id: StepId,
    attempt: u32,
    execution_status: AgentExecutionStatus,
    liveness: AgentLiveness,
    source: AgentStatusSource,
    requested_model: Option<String>,
    requested_reasoning_effort: Option<String>,
    actual_model: Option<String>,
    actual_reasoning_effort: Option<String>,
    activity: String,
    blocker: Option<String>,
    dependency: Option<String>,
    task_summary: String,
    evidence_references: Vec<String>,
    usage_kind: Option<AgentUsageKind>,
    usage_value: Option<u64>,
    observed_at: OffsetDateTime,
    report_sequence: u64,
    idempotency_key: String,
    claim_fence: StepClaimFence,
}

#[allow(clippy::too_many_arguments)]
impl CeremonyAgentStatus {
    pub fn new(
        ceremony_id: CeremonyId,
        agent_execution_id: CeremonyAgentExecutionId,
        operation_id: ExecutionOperationId,
        claim_owner_id: LeaseOwnerId,
        logical_worker_id: LogicalWorkerId,
        host_agent_id: LeaseOwnerId,
        host_agent_incarnation: HostAgentIncarnation,
        previous_host_agent_id: Option<LeaseOwnerId>,
        previous_host_agent_incarnation: Option<HostAgentIncarnation>,
        role_id: RoleId,
        step_id: StepId,
        attempt: u32,
        execution_status: AgentExecutionStatus,
        liveness: AgentLiveness,
        source: AgentStatusSource,
        requested_model: Option<String>,
        requested_reasoning_effort: Option<String>,
        actual_model: Option<String>,
        actual_reasoning_effort: Option<String>,
        activity: impl Into<String>,
        blocker: Option<String>,
        dependency: Option<String>,
        task_summary: impl Into<String>,
        evidence_references: Vec<String>,
        usage_kind: Option<AgentUsageKind>,
        usage_value: Option<u64>,
        observed_at: OffsetDateTime,
        report_sequence: u64,
        idempotency_key: impl Into<String>,
        claim_fence: StepClaimFence,
    ) -> Result<Self, DomainError> {
        if report_sequence == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "report_sequence",
            });
        }
        let status = Self {
            ceremony_id,
            agent_execution_id,
            operation_id,
            claim_owner_id,
            logical_worker_id,
            host_agent_id,
            host_agent_incarnation,
            previous_host_agent_id,
            previous_host_agent_incarnation,
            role_id,
            step_id,
            attempt,
            execution_status,
            liveness,
            source,
            requested_model: optional_text(requested_model, "requested_model")?,
            requested_reasoning_effort: optional_text(
                requested_reasoning_effort,
                "requested_reasoning_effort",
            )?,
            actual_model: optional_text(actual_model, "actual_model")?,
            actual_reasoning_effort: optional_text(
                actual_reasoning_effort,
                "actual_reasoning_effort",
            )?,
            activity: text(activity.into(), "activity")?,
            blocker: optional_text(blocker, "blocker")?,
            dependency: optional_text(dependency, "dependency")?,
            task_summary: text(task_summary.into(), "task_summary")?,
            evidence_references: evidence_references
                .into_iter()
                .map(|value| text(value, "evidence_references"))
                .collect::<Result<_, _>>()?,
            usage_kind,
            usage_value,
            observed_at,
            report_sequence,
            idempotency_key: text(idempotency_key.into(), "idempotency_key")?,
            claim_fence,
        };
        if status.previous_host_agent_id.is_some()
            != status.previous_host_agent_incarnation.is_some()
        {
            return Err(DomainError::InvariantViolated {
                reason: "handoff provenance must include both previous host identities",
            });
        }
        if status.evidence_references.len() > MAX_EVIDENCE_REFERENCES {
            return Err(DomainError::OutOfRange {
                field: "evidence_references",
                value: status.evidence_references.len() as f64,
                min: 0.0,
                max: MAX_EVIDENCE_REFERENCES as f64,
            });
        }
        match (status.usage_kind, status.usage_value) {
            (None | Some(AgentUsageKind::Unavailable), None)
            | (Some(AgentUsageKind::Measured | AgentUsageKind::Estimated), Some(_)) => {}
            _ => {
                return Err(DomainError::InvariantViolated {
                    reason: "agent usage value must match measured, estimated, or unavailable provenance",
                });
            }
        }
        // A lease proves only that MADE has not accepted a competing claim. It
        // says nothing about whether an external host is still reachable.
        if matches!(status.source, AgentStatusSource::DerivedLease)
            && matches!(
                status.liveness,
                AgentLiveness::Fresh | AgentLiveness::Unreachable
            )
        {
            return Err(DomainError::InvariantViolated {
                reason: "a derived lease cannot claim host liveness",
            });
        }
        if matches!(status.source, AgentStatusSource::HostDiscoveryUnsupported)
            && !matches!(status.liveness, AgentLiveness::Unknown)
        {
            return Err(DomainError::InvariantViolated {
                reason: "an unsupported host can only yield an unknown observation",
            });
        }
        Ok(status)
    }

    #[must_use]
    pub fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }
    #[must_use]
    pub fn agent_execution_id(&self) -> &CeremonyAgentExecutionId {
        &self.agent_execution_id
    }
    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
    }
    #[must_use]
    pub const fn claim_owner_id(&self) -> &LeaseOwnerId {
        &self.claim_owner_id
    }
    #[must_use]
    pub fn logical_worker_id(&self) -> &LogicalWorkerId {
        &self.logical_worker_id
    }
    #[must_use]
    pub fn host_agent_id(&self) -> &LeaseOwnerId {
        &self.host_agent_id
    }
    #[must_use]
    pub fn host_agent_incarnation(&self) -> &HostAgentIncarnation {
        &self.host_agent_incarnation
    }
    #[must_use]
    pub fn previous_host_agent_id(&self) -> Option<&LeaseOwnerId> {
        self.previous_host_agent_id.as_ref()
    }
    #[must_use]
    pub fn previous_host_agent_incarnation(&self) -> Option<&HostAgentIncarnation> {
        self.previous_host_agent_incarnation.as_ref()
    }
    #[must_use]
    pub fn role_id(&self) -> &RoleId {
        &self.role_id
    }
    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }
    #[must_use]
    pub fn attempt(&self) -> u32 {
        self.attempt
    }
    #[must_use]
    pub fn execution_status(&self) -> AgentExecutionStatus {
        self.execution_status
    }
    #[must_use]
    pub fn liveness(&self) -> AgentLiveness {
        self.liveness
    }
    #[must_use]
    pub fn source(&self) -> AgentStatusSource {
        self.source
    }
    #[must_use]
    pub fn requested_model(&self) -> Option<&str> {
        self.requested_model.as_deref()
    }
    #[must_use]
    pub fn requested_reasoning_effort(&self) -> Option<&str> {
        self.requested_reasoning_effort.as_deref()
    }
    #[must_use]
    pub fn actual_model(&self) -> Option<&str> {
        self.actual_model.as_deref()
    }
    #[must_use]
    pub fn actual_reasoning_effort(&self) -> Option<&str> {
        self.actual_reasoning_effort.as_deref()
    }
    #[must_use]
    pub fn activity(&self) -> &str {
        &self.activity
    }
    #[must_use]
    pub fn blocker(&self) -> Option<&str> {
        self.blocker.as_deref()
    }
    #[must_use]
    pub fn dependency(&self) -> Option<&str> {
        self.dependency.as_deref()
    }
    #[must_use]
    pub fn task_summary(&self) -> &str {
        &self.task_summary
    }
    #[must_use]
    pub fn evidence_references(&self) -> &[String] {
        &self.evidence_references
    }
    #[must_use]
    pub const fn usage_kind(&self) -> Option<AgentUsageKind> {
        self.usage_kind
    }
    #[must_use]
    pub fn usage_value(&self) -> Option<u64> {
        self.usage_value
    }
    #[must_use]
    pub fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }
    #[must_use]
    pub fn report_sequence(&self) -> u64 {
        self.report_sequence
    }
    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }
    #[must_use]
    pub const fn claim_fence(&self) -> &StepClaimFence {
        &self.claim_fence
    }

    /// Project liveness from the age of explicit host evidence without
    /// changing the execution outcome or the persisted observation.
    #[must_use]
    pub fn projected_at(&self, now: OffsetDateTime, stale_after: Duration) -> Self {
        let mut projected = self.clone();
        if self.source == AgentStatusSource::HostReport
            && self.liveness == AgentLiveness::Fresh
            && now - self.observed_at > stale_after
        {
            projected.liveness = AgentLiveness::Stale;
        }
        projected
    }

    /// Identity that belongs to the ceremony's logical participant and its
    /// accepted work, rather than to the host process currently executing it.
    #[must_use]
    pub fn has_same_logical_execution_as(&self, other: &Self) -> bool {
        self.ceremony_id == other.ceremony_id
            && self.agent_execution_id == other.agent_execution_id
            && self.logical_worker_id == other.logical_worker_id
            && self.role_id == other.role_id
            && self.step_id == other.step_id
            && self.attempt == other.attempt
    }
}

#[allow(clippy::needless_pass_by_value)] // the validated value is retained by the entity
fn text(value: String, field: &'static str) -> Result<String, DomainError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    if value.len() > MAX_STATUS_TEXT {
        return Err(DomainError::FieldTooLong {
            field,
            actual: value.len(),
            max: MAX_STATUS_TEXT,
        });
    }
    if value.chars().any(char::is_control) {
        return Err(DomainError::InvalidCharacters { field });
    }
    Ok(value)
}

fn optional_text(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, DomainError> {
    value.map(|value| text(value, field)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation_id() -> ExecutionOperationId {
        ExecutionOperationId::new("1".repeat(64)).unwrap()
    }

    fn claim_owner_id() -> LeaseOwnerId {
        LeaseOwnerId::new("host").unwrap()
    }

    fn claim_fence() -> StepClaimFence {
        StepClaimFence::new("2".repeat(64)).unwrap()
    }

    fn ceremony(value: &str) -> CeremonyId {
        CeremonyId::new(value).unwrap()
    }
    fn execution(value: &str) -> CeremonyAgentExecutionId {
        CeremonyAgentExecutionId::new(value).unwrap()
    }
    fn worker(value: &str) -> LogicalWorkerId {
        LogicalWorkerId::new(value).unwrap()
    }
    fn owner(value: &str) -> LeaseOwnerId {
        LeaseOwnerId::new(value).unwrap()
    }
    fn incarnation(value: &str) -> HostAgentIncarnation {
        HostAgentIncarnation::new(value).unwrap()
    }
    fn role(value: &str) -> RoleId {
        RoleId::new(value).unwrap()
    }
    fn step(value: &str) -> StepId {
        StepId::new(value).unwrap()
    }

    fn status_with_usage(
        usage_kind: Option<AgentUsageKind>,
        usage_value: Option<u64>,
    ) -> Result<CeremonyAgentStatus, DomainError> {
        CeremonyAgentStatus::new(
            ceremony("ceremony"),
            execution("execution"),
            operation_id(),
            claim_owner_id(),
            worker("worker"),
            owner("host"),
            incarnation("inc"),
            None,
            None,
            role("reviewer"),
            step("step"),
            1,
            AgentExecutionStatus::Blocked,
            AgentLiveness::Fresh,
            AgentStatusSource::HostReport,
            None,
            None,
            None,
            None,
            "waiting for build lock",
            Some("build lock".into()),
            None,
            "bounded summary",
            vec![],
            usage_kind,
            usage_value,
            OffsetDateTime::UNIX_EPOCH,
            1,
            "report-1",
            claim_fence(),
        )
    }

    #[test]
    fn execution_and_liveness_are_independent() {
        let status = status_with_usage(Some(AgentUsageKind::Unavailable), None).unwrap();
        assert_eq!(status.execution_status(), AgentExecutionStatus::Blocked);
        assert_eq!(status.liveness(), AgentLiveness::Fresh);
        let aged = status.projected_at(
            OffsetDateTime::UNIX_EPOCH + Duration::seconds(61),
            Duration::seconds(60),
        );
        assert_eq!(aged.execution_status(), AgentExecutionStatus::Blocked);
        assert_eq!(aged.liveness(), AgentLiveness::Stale);
        assert_eq!(aged.source(), AgentStatusSource::HostReport);
    }

    #[test]
    fn usage_value_matches_its_provenance() {
        assert!(status_with_usage(Some(AgentUsageKind::Measured), Some(7)).is_ok());
        assert!(status_with_usage(Some(AgentUsageKind::Estimated), Some(7)).is_ok());
        assert!(status_with_usage(Some(AgentUsageKind::Unavailable), None).is_ok());
        assert!(status_with_usage(Some(AgentUsageKind::Measured), None).is_err());
        assert!(status_with_usage(Some(AgentUsageKind::Unavailable), Some(7)).is_err());
        assert!(status_with_usage(None, Some(7)).is_err());
    }

    #[test]
    fn handoff_requires_both_previous_identities() {
        assert!(CeremonyAgentStatus::new(
            ceremony("c"),
            execution("e"),
            operation_id(),
            claim_owner_id(),
            worker("w"),
            owner("h"),
            incarnation("i"),
            Some(owner("old")),
            None,
            role("r"),
            step("s"),
            1,
            AgentExecutionStatus::Running,
            AgentLiveness::Fresh,
            AgentStatusSource::HostReport,
            None,
            None,
            None,
            None,
            "work",
            None,
            None,
            "summary",
            vec![],
            None,
            None,
            OffsetDateTime::UNIX_EPOCH,
            1,
            "k",
            claim_fence(),
        )
        .is_err());
    }

    #[test]
    fn a_lease_or_unsupported_host_never_fabricates_a_fresh_observation() {
        for source in [
            AgentStatusSource::DerivedLease,
            AgentStatusSource::HostDiscoveryUnsupported,
        ] {
            assert!(CeremonyAgentStatus::new(
                ceremony("c"),
                execution("e"),
                operation_id(),
                claim_owner_id(),
                worker("participant"),
                owner("host"),
                incarnation("inc"),
                None,
                None,
                role("role"),
                step("step"),
                1,
                AgentExecutionStatus::Running,
                AgentLiveness::Fresh,
                source,
                None,
                None,
                None,
                None,
                "working",
                None,
                None,
                "summary",
                vec![],
                None,
                None,
                OffsetDateTime::UNIX_EPOCH,
                1,
                "key",
                claim_fence(),
            )
            .is_err());
        }
        assert!(CeremonyAgentStatus::new(
            ceremony("c"),
            execution("e"),
            operation_id(),
            claim_owner_id(),
            worker("participant"),
            owner("host"),
            incarnation("inc"),
            None,
            None,
            role("role"),
            step("step"),
            1,
            AgentExecutionStatus::Blocked,
            AgentLiveness::Stale,
            AgentStatusSource::DerivedLease,
            None,
            None,
            None,
            None,
            "waiting",
            Some("host report missing".into()),
            None,
            "summary",
            vec![],
            None,
            None,
            OffsetDateTime::UNIX_EPOCH,
            1,
            "key",
            claim_fence(),
        )
        .is_ok());
    }
}
