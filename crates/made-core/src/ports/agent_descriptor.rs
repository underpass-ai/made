use serde::{Deserialize, Serialize};

use crate::value_objects::{AgentId, AgentKind, Attributes, Specialty};

/// Domain description used to construct a live agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDescriptor {
    pub id: AgentId,
    pub specialty: Specialty,
    pub kind: AgentKind,
    pub attributes: Attributes,
}
