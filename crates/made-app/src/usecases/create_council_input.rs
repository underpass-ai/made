use made_core::value_objects::{AgentId, CouncilId, Specialty};

/// Input for creating a council after its agents are registered.
#[derive(Debug, Clone)]
pub struct CreateCouncilInput {
    pub council_id: CouncilId,
    pub specialty: Specialty,
    pub agents: Vec<AgentId>,
}
