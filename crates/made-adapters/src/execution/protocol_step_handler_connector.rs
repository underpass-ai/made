use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    ClockPort, ProtocolExecutionConnectorOutcome, ProtocolExecutionConnectorPort,
    ProtocolExecutionObservation, ProtocolExecutionRequest, ProtocolStepHandlerPort,
};
use made_core::value_objects::{
    ArtifactSourceKind, ExecutionConnectorId, ExecutionRecoveryCapability,
};

/// Compatibility connector for handlers with no authoritative recovery API.
pub struct ProtocolStepHandlerConnector {
    id: ExecutionConnectorId,
    source_kind: ArtifactSourceKind,
    handler: Arc<dyn ProtocolStepHandlerPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for ProtocolStepHandlerConnector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProtocolStepHandlerConnector")
            .field("id", &self.id)
            .field("source_kind", &self.source_kind)
            .finish_non_exhaustive()
    }
}

impl ProtocolStepHandlerConnector {
    pub fn new(
        id: ExecutionConnectorId,
        source_kind: ArtifactSourceKind,
        handler: Arc<dyn ProtocolStepHandlerPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Result<Self, DomainError> {
        if !source_kind.is_execution_source() {
            return Err(DomainError::InvariantViolated {
                reason: "step handler connector requires an execution source kind",
            });
        }
        Ok(Self {
            id,
            source_kind,
            handler,
            clock,
        })
    }
}

#[async_trait]
impl ProtocolExecutionConnectorPort for ProtocolStepHandlerConnector {
    fn connector_id(&self) -> &ExecutionConnectorId {
        &self.id
    }

    fn recovery_capability(&self) -> ExecutionRecoveryCapability {
        ExecutionRecoveryCapability::ReconciliationRequired
    }

    fn source_kind(&self) -> ArtifactSourceKind {
        self.source_kind
    }

    async fn execute_or_recover(
        &self,
        request: ProtocolExecutionRequest,
    ) -> Result<ProtocolExecutionConnectorOutcome, DomainError> {
        let producer_claim_fence = request.intent().claim_fence().clone();
        let result = self
            .handler
            .execute(request.handler_request().clone())
            .await?;
        Ok(ProtocolExecutionConnectorOutcome::Observed(Box::new(
            ProtocolExecutionObservation::new(
                producer_claim_fence,
                None,
                result,
                Vec::new(),
                self.clock.now(),
            ),
        )))
    }
}
