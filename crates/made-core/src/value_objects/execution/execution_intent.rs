use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{ExecutionConnectorId, ExecutionOperation};
use crate::value_objects::artifact::ArtifactSourceKind;
use crate::value_objects::audit::AuditActorKind;
use crate::value_objects::ceremony::StepClaimFence;

/// Durable declaration written immediately before one claim calls a connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionIntent {
    operation: ExecutionOperation,
    claim_fence: StepClaimFence,
    connector_id: ExecutionConnectorId,
    source_kind: ArtifactSourceKind,
    actor_kind: AuditActorKind,
    #[serde(with = "time::serde::rfc3339")]
    recorded_at: OffsetDateTime,
}

impl ExecutionIntent {
    pub fn new(
        operation: ExecutionOperation,
        claim_fence: StepClaimFence,
        connector_id: ExecutionConnectorId,
        source_kind: ArtifactSourceKind,
        actor_kind: AuditActorKind,
        recorded_at: OffsetDateTime,
    ) -> Result<Self, crate::error::DomainError> {
        if !source_kind.is_execution_source() {
            return Err(crate::error::DomainError::InvariantViolated {
                reason: "execution intent requires an execution source kind",
            });
        }
        Ok(Self {
            operation,
            claim_fence,
            connector_id,
            source_kind,
            actor_kind,
            recorded_at,
        })
    }

    #[must_use]
    pub const fn operation(&self) -> &ExecutionOperation {
        &self.operation
    }

    #[must_use]
    pub const fn claim_fence(&self) -> &StepClaimFence {
        &self.claim_fence
    }

    #[must_use]
    pub const fn connector_id(&self) -> &ExecutionConnectorId {
        &self.connector_id
    }

    #[must_use]
    pub const fn source_kind(&self) -> ArtifactSourceKind {
        self.source_kind
    }

    #[must_use]
    pub const fn actor_kind(&self) -> AuditActorKind {
        self.actor_kind
    }

    #[must_use]
    pub const fn recorded_at(&self) -> OffsetDateTime {
        self.recorded_at
    }
}
