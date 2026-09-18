use async_trait::async_trait;

use super::{CeremonyExecutionObservation, CeremonyExecutionRequest};
use crate::error::DomainError;
use crate::value_objects::{
    ArtifactSourceKind, ExecutionConnectorId, ExecutionIntent, ExecutionRecoveryCapability,
};

/// Executes or authoritatively recovers a semantic operation for one claim.
#[async_trait]
pub trait CeremonyExecutionConnectorPort: Send + Sync {
    fn connector_id(&self) -> &ExecutionConnectorId;

    fn recovery_capability(&self) -> ExecutionRecoveryCapability;

    fn source_kind(&self) -> ArtifactSourceKind;

    async fn execute_or_recover(
        &self,
        request: CeremonyExecutionRequest,
    ) -> Result<CeremonyExecutionObservation, DomainError>;

    /// Settle a durable intent after the handler-shaped request was lost with the process.
    async fn recover_intent(
        &self,
        _intent: &ExecutionIntent,
    ) -> Result<CeremonyExecutionObservation, DomainError> {
        Err(DomainError::InvariantViolated {
            reason: "execution connector cannot recover from a sealed intent",
        })
    }
}
