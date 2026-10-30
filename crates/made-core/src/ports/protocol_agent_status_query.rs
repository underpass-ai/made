use crate::entities::AgentExecutionStatus;
use crate::error::DomainError;

/// Bounded, filtered keyset query over one protocol's live agent roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolAgentStatusQuery {
    protocol_id: String,
    cursor: Option<String>,
    limit: usize,
    execution_status: Option<AgentExecutionStatus>,
}

impl ProtocolAgentStatusQuery {
    pub fn new(
        protocol_id: impl Into<String>,
        cursor: Option<String>,
        limit: usize,
        execution_status: Option<AgentExecutionStatus>,
    ) -> Result<Self, DomainError> {
        if limit == 0 || limit > 100 {
            return Err(DomainError::OutOfRange {
                field: "agent_status.limit",
                value: limit as f64,
                min: 1.0,
                max: 100.0,
            });
        }
        Ok(Self {
            protocol_id: protocol_id.into(),
            cursor,
            limit,
            execution_status,
        })
    }

    #[must_use]
    pub fn protocol_id(&self) -> &str {
        &self.protocol_id
    }
    #[must_use]
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }
    #[must_use]
    pub fn limit(&self) -> usize {
        self.limit
    }
    #[must_use]
    pub fn execution_status(&self) -> Option<AgentExecutionStatus> {
        self.execution_status
    }
}
