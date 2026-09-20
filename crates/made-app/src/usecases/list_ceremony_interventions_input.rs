use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyAgentExecutionId, CeremonyId, CeremonyInterventionId, RoleId,
};

const MAX_LIMIT: usize = 100;

/// A bounded, filtered look at one ceremony's interventions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListCeremonyInterventionsInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) status: Option<String>,
    pub(crate) role_id: Option<RoleId>,
    pub(crate) agent_execution_id: Option<CeremonyAgentExecutionId>,
    pub(crate) unresolved_only: bool,
    pub(crate) limit: usize,
    pub(crate) cursor: Option<CeremonyInterventionId>,
}

impl ListCeremonyInterventionsInput {
    #[must_use]
    pub fn new(instance_id: CeremonyId) -> Self {
        Self {
            instance_id,
            status: None,
            role_id: None,
            agent_execution_id: None,
            unresolved_only: false,
            limit: MAX_LIMIT,
            cursor: None,
        }
    }

    /// Only items whose projected delivery status has this name.
    #[must_use]
    pub fn in_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    #[must_use]
    pub fn for_role(mut self, role_id: RoleId) -> Self {
        self.role_id = Some(role_id);
        self
    }

    #[must_use]
    pub fn for_agent_execution(mut self, agent_execution_id: CeremonyAgentExecutionId) -> Self {
        self.agent_execution_id = Some(agent_execution_id);
        self
    }

    #[must_use]
    pub const fn unresolved_only(mut self, unresolved_only: bool) -> Self {
        self.unresolved_only = unresolved_only;
        self
    }

    pub fn of_size(mut self, limit: usize) -> Result<Self, DomainError> {
        if limit == 0 || limit > MAX_LIMIT {
            return Err(DomainError::OutOfRange {
                field: "list_ceremony_interventions.limit",
                value: limit as f64,
                min: 1.0,
                max: MAX_LIMIT as f64,
            });
        }
        self.limit = limit;
        Ok(self)
    }

    #[must_use]
    pub fn after(mut self, cursor: CeremonyInterventionId) -> Self {
        self.cursor = Some(cursor);
        self
    }

    #[must_use]
    pub const fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }
}
