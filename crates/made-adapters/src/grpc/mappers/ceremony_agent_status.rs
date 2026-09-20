use made_core::entities::{
    AgentExecutionStatus, AgentLiveness, AgentStatusSource, AgentUsageKind, CeremonyAgentStatus,
};
use made_core::error::DomainError;
use made_core::ports::{CeremonyAgentStatusPage, CeremonyAgentStatusQuery};
use made_core::value_objects::{
    CeremonyAgentExecutionId, CeremonyId, ExecutionOperationId, HostAgentIncarnation, LeaseOwnerId,
    LogicalWorkerId, RoleId, StepClaimFence, StepId,
};
use made_proto::v1 as pb;

use super::timestamp::{offset_to_timestamp, timestamp_to_offset};

pub fn status_from_proto(
    input: pb::CeremonyAgentStatus,
) -> Result<CeremonyAgentStatus, DomainError> {
    let observed_at = input.observed_at.ok_or(DomainError::EmptyField {
        field: "status.observed_at",
    })?;
    CeremonyAgentStatus::new(
        CeremonyId::new(input.ceremony_id)?,
        CeremonyAgentExecutionId::new(input.agent_execution_id)?,
        ExecutionOperationId::new(input.operation_id)?,
        LeaseOwnerId::new(input.claim_owner_id)?,
        LogicalWorkerId::new(input.logical_worker_id)?,
        LeaseOwnerId::new(input.host_agent_id)?,
        HostAgentIncarnation::new(input.host_agent_incarnation)?,
        input
            .previous_host_agent_id
            .map(LeaseOwnerId::new)
            .transpose()?,
        input
            .previous_host_agent_incarnation
            .map(HostAgentIncarnation::new)
            .transpose()?,
        RoleId::new(input.role_id)?,
        StepId::new(input.step_id)?,
        input.attempt,
        parse_execution_status(&input.execution_status)?,
        parse_liveness(&input.liveness)?,
        parse_source(&input.source)?,
        input.requested_model,
        input.requested_reasoning_effort,
        input.actual_model,
        input.actual_reasoning_effort,
        input.activity,
        input.blocker,
        input.dependency,
        input.task_summary,
        input.evidence_references,
        input
            .usage_kind
            .as_deref()
            .map(AgentUsageKind::try_from)
            .transpose()?,
        input.usage_value,
        timestamp_to_offset(observed_at)?,
        input.report_sequence,
        input.idempotency_key,
        StepClaimFence::new(input.claim_fence)?,
    )
}

pub fn status_to_proto(status: &CeremonyAgentStatus) -> pb::CeremonyAgentStatus {
    pb::CeremonyAgentStatus {
        ceremony_id: status.ceremony_id().as_str().to_owned(),
        agent_execution_id: status.agent_execution_id().as_str().to_owned(),
        operation_id: status.operation_id().as_str().to_owned(),
        claim_owner_id: status.claim_owner_id().as_str().to_owned(),
        logical_worker_id: status.logical_worker_id().as_str().to_owned(),
        host_agent_id: status.host_agent_id().as_str().to_owned(),
        host_agent_incarnation: status.host_agent_incarnation().as_str().to_owned(),
        previous_host_agent_id: status
            .previous_host_agent_id()
            .map(|value| value.as_str().to_owned()),
        previous_host_agent_incarnation: status
            .previous_host_agent_incarnation()
            .map(|value| value.as_str().to_owned()),
        role_id: status.role_id().as_str().to_owned(),
        step_id: status.step_id().as_str().to_owned(),
        attempt: status.attempt(),
        execution_status: serde_json::to_value(status.execution_status())
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned(),
        liveness: serde_json::to_value(status.liveness())
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned(),
        source: serde_json::to_value(status.source())
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned(),
        requested_model: status.requested_model().map(str::to_owned),
        requested_reasoning_effort: status.requested_reasoning_effort().map(str::to_owned),
        actual_model: status.actual_model().map(str::to_owned),
        actual_reasoning_effort: status.actual_reasoning_effort().map(str::to_owned),
        activity: status.activity().to_owned(),
        blocker: status.blocker().map(str::to_owned),
        dependency: status.dependency().map(str::to_owned),
        task_summary: status.task_summary().to_owned(),
        evidence_references: status.evidence_references().to_vec(),
        usage_kind: status.usage_kind().map(|value| {
            serde_json::to_value(value)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        }),
        usage_value: status.usage_value(),
        observed_at: Some(offset_to_timestamp(status.observed_at())),
        report_sequence: status.report_sequence(),
        idempotency_key: status.idempotency_key().to_owned(),
        claim_fence: status.claim_fence().as_str().to_owned(),
    }
}

pub fn query_from_proto(
    input: pb::ListCeremonyAgentsRequest,
) -> Result<CeremonyAgentStatusQuery, DomainError> {
    CeremonyAgentStatusQuery::new(
        input.ceremony_id,
        input.cursor,
        usize::try_from(if input.limit == 0 { 50 } else { input.limit }).unwrap_or(100),
        input
            .execution_status
            .as_deref()
            .map(parse_execution_status)
            .transpose()?,
    )
}

pub fn page_to_proto(page: &CeremonyAgentStatusPage) -> pb::ListCeremonyAgentsResponse {
    pb::ListCeremonyAgentsResponse {
        agents: page.entries().iter().map(status_to_proto).collect(),
        next_cursor: page.next_cursor().map(str::to_owned),
    }
}

fn parse_execution_status(value: &str) -> Result<AgentExecutionStatus, DomainError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|_| {
        DomainError::InvariantViolated {
            reason: "unsupported agent execution status",
        }
    })
}
fn parse_liveness(value: &str) -> Result<AgentLiveness, DomainError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|_| {
        DomainError::InvariantViolated {
            reason: "unsupported agent liveness",
        }
    })
}
fn parse_source(value: &str) -> Result<AgentStatusSource, DomainError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|_| {
        DomainError::InvariantViolated {
            reason: "unsupported agent status source",
        }
    })
}
