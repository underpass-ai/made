use made_core::ports::AgentDescriptor;
use made_core::value_objects::{AgentId, AgentKind, Attributes, Specialty};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyParticipantDescriptor {
    id: AgentId,
    specialty: Specialty,
    kind: AgentKind,
    attributes: Attributes,
}

impl CeremonyParticipantDescriptor {
    #[must_use]
    pub fn new(id: AgentId, specialty: Specialty, kind: AgentKind, attributes: Attributes) -> Self {
        Self {
            id,
            specialty,
            kind,
            attributes,
        }
    }

    #[must_use]
    pub fn id(&self) -> &AgentId {
        &self.id
    }

    #[must_use]
    pub fn specialty(&self) -> &Specialty {
        &self.specialty
    }

    #[must_use]
    pub fn kind(&self) -> &AgentKind {
        &self.kind
    }

    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    #[must_use]
    pub fn into_agent_descriptor(self) -> AgentDescriptor {
        AgentDescriptor {
            id: self.id,
            specialty: self.specialty,
            kind: self.kind,
            attributes: self.attributes,
        }
    }
}
