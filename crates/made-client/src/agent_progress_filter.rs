/// Optional filters for one ceremony's global agent activity feed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentProgressFilter {
    role: Option<String>,
    step: Option<String>,
    agent_execution: Option<String>,
}

impl AgentProgressFilter {
    #[must_use]
    pub fn new(
        role_id: Option<String>,
        step_id: Option<String>,
        agent_execution_id: Option<String>,
    ) -> Self {
        Self {
            role: role_id,
            step: step_id,
            agent_execution: agent_execution_id,
        }
    }

    #[must_use]
    pub fn role_id(&self) -> Option<&str> {
        self.role.as_deref()
    }
    #[must_use]
    pub fn step_id(&self) -> Option<&str> {
        self.step.as_deref()
    }
    #[must_use]
    pub fn agent_execution_id(&self) -> Option<&str> {
        self.agent_execution.as_deref()
    }
}
