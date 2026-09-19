use async_trait::async_trait;

use super::{CeremonyExecutionConnectorOutcome, CeremonyExecutionRequest, ExecutionCancellation};
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
    ) -> Result<CeremonyExecutionConnectorOutcome, DomainError>;

    /// Default for adapters without owned processes. Process/container adapters
    /// override this to kill and reap their whole execution before returning.
    async fn execute_cancellable(
        &self,
        request: CeremonyExecutionRequest,
        cancellation: ExecutionCancellation,
    ) -> Result<CeremonyExecutionConnectorOutcome, DomainError> {
        cancellation
            .run(self.execute_or_recover(request))
            .await
            .ok_or(DomainError::InvariantViolated {
                reason: "execution authority was cancelled",
            })?
    }

    /// Settle a durable intent after the handler-shaped request was lost with the process.
    async fn recover_intent(
        &self,
        intent: &ExecutionIntent,
    ) -> Result<CeremonyExecutionConnectorOutcome, DomainError> {
        Ok(CeremonyExecutionConnectorOutcome::ReconciliationRequired(
            intent.operation().operation_id().clone(),
        ))
    }
}
