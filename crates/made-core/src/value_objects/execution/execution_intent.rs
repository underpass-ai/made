use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{ExecutionConnectorId, ExecutionOperation, ExecutionRecoveryCapability};
use crate::error::DomainError;
use crate::value_objects::artifact::ArtifactSourceKind;
use crate::value_objects::audit::AuditActorKind;
use crate::value_objects::ceremony::StepClaimFence;

/// Durable declaration written immediately before one claim calls a connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionIntent {
    operation: ExecutionOperation,
    claim_fence: StepClaimFence,
    connector_id: ExecutionConnectorId,
    recovery_capability: ExecutionRecoveryCapability,
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
        recovery_capability: ExecutionRecoveryCapability,
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
            recovery_capability,
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
    pub const fn recovery_capability(&self) -> ExecutionRecoveryCapability {
        self.recovery_capability
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

    /// Re-check nested and cross-field invariants after deserialization.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.operation.validate()?;
        if !self.source_kind.is_execution_source() {
            return Err(DomainError::InvariantViolated {
                reason: "execution intent requires an execution source kind",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::value_objects::{CeremonyId, StateIteration, StateVisit, StepId, StepIteration};

    #[test]
    fn deserialized_intent_rejects_a_non_execution_source() {
        let operation = ExecutionOperation::new(
            CeremonyId::new("ceremony").unwrap(),
            StepId::new("work").unwrap(),
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
            super::super::ExecutionRequestBytes::new(b"request".to_vec()).unwrap(),
        );
        let mut raw = serde_json::to_value(
            ExecutionIntent::new(
                operation,
                StepClaimFence::new("1".repeat(64)).unwrap(),
                ExecutionConnectorId::new("test").unwrap(),
                ExecutionRecoveryCapability::IdempotentByOperationId,
                ArtifactSourceKind::NoOp,
                AuditActorKind::Engine,
                OffsetDateTime::UNIX_EPOCH,
            )
            .unwrap(),
        )
        .unwrap();
        raw["source_kind"] = json!("generated_report");
        let corrupted: ExecutionIntent = serde_json::from_value(raw).unwrap();
        assert!(corrupted.validate().is_err());
    }
}
