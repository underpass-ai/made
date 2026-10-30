use crate::entities::ProtocolEvidencePack;
use crate::error::DomainError;
use crate::value_objects::{
    ProtocolContext, ProtocolEvidenceSourceId, ProtocolId, ProtocolInterventionContent,
    ProtocolInterventionId, RoleId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolEvidenceRequest {
    instance_id: ProtocolId,
    intervention_id: ProtocolInterventionId,
    role_id: RoleId,
    source_id: ProtocolEvidenceSourceId,
    query: ProtocolInterventionContent,
    context: ProtocolContext,
}

impl ProtocolEvidenceRequest {
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        intervention_id: ProtocolInterventionId,
        role_id: RoleId,
        source_id: ProtocolEvidenceSourceId,
        query: ProtocolInterventionContent,
        context: ProtocolContext,
    ) -> Self {
        Self {
            instance_id,
            intervention_id,
            role_id,
            source_id,
            query,
            context,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }

    #[must_use]
    pub fn intervention_id(&self) -> &ProtocolInterventionId {
        &self.intervention_id
    }

    #[must_use]
    pub fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    #[must_use]
    pub fn source_id(&self) -> &ProtocolEvidenceSourceId {
        &self.source_id
    }

    #[must_use]
    pub fn query(&self) -> &ProtocolInterventionContent {
        &self.query
    }

    #[must_use]
    pub fn context(&self) -> &ProtocolContext {
        &self.context
    }

    pub fn ensure_matches(&self, pack: &ProtocolEvidencePack) -> Result<(), DomainError> {
        if pack.source_id() == &self.source_id {
            Ok(())
        } else {
            Err(DomainError::InvariantViolated {
                reason: "protocol evidence source returned a pack for another source",
            })
        }
    }
}
