use crate::error::DomainError;
use crate::value_objects::{CeremonyAgentExecutionId, CeremonyId, RoleId, StepId};

/// Global-cursor query with optional filters that never renumber the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyAgentActivityQuery {
    ceremony_id: CeremonyId,
    after_sequence: u64,
    limit: usize,
    role_id: Option<RoleId>,
    step_id: Option<StepId>,
    agent_execution_id: Option<CeremonyAgentExecutionId>,
}

impl CeremonyAgentActivityQuery {
    pub fn new(
        ceremony_id: CeremonyId,
        after_sequence: u64,
        limit: usize,
        role_id: Option<RoleId>,
        step_id: Option<StepId>,
        agent_execution_id: Option<CeremonyAgentExecutionId>,
    ) -> Result<Self, DomainError> {
        if limit == 0 || limit > 1_000 {
            return Err(DomainError::OutOfRange {
                field: "agent_activity.limit",
                value: limit as f64,
                min: 1.0,
                max: 1_000.0,
            });
        }
        Ok(Self {
            ceremony_id,
            after_sequence,
            limit,
            role_id,
            step_id,
            agent_execution_id,
        })
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }
    #[must_use]
    pub const fn after_sequence(&self) -> u64 {
        self.after_sequence
    }
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }
    #[must_use]
    pub const fn role_id(&self) -> Option<&RoleId> {
        self.role_id.as_ref()
    }
    #[must_use]
    pub const fn step_id(&self) -> Option<&StepId> {
        self.step_id.as_ref()
    }
    #[must_use]
    pub const fn agent_execution_id(&self) -> Option<&CeremonyAgentExecutionId> {
        self.agent_execution_id.as_ref()
    }
}
