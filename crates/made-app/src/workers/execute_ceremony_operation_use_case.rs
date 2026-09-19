use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{
    CeremonyExecutionConnectorOutcome, CeremonyExecutionConnectorPort, CeremonyExecutionRequest,
    ClockPort, ExecutionCancellation, ExecutionReceiptStorePort, RecordExecutionIntentOutcome,
};
use made_core::value_objects::{
    ExecutionIntent, ExecutionOperation, ExecutionReconciliationRequirement,
    ExecutionRecoveryCapability,
};

use super::execution_receipt_artifact_verifier::verify_receipt_artifacts;
use super::execution_receipt_from_observation::execution_receipt_from_observation;
use super::{ExecuteCeremonyOperationInput, ExecuteCeremonyOperationOutcome};
use crate::artifacts::ArtifactService;

/// Persist intent, execute or recover once, then persist an immutable receipt.
pub struct ExecuteCeremonyOperationUseCase {
    store: Arc<dyn ExecutionReceiptStorePort>,
    connector: Arc<dyn CeremonyExecutionConnectorPort>,
    clock: Arc<dyn ClockPort>,
    artifacts: Option<Arc<ArtifactService>>,
}

impl std::fmt::Debug for ExecuteCeremonyOperationUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecuteCeremonyOperationUseCase")
            .field("connector_id", &self.connector.connector_id())
            .finish_non_exhaustive()
    }
}

impl ExecuteCeremonyOperationUseCase {
    #[must_use]
    pub fn new(
        store: Arc<dyn ExecutionReceiptStorePort>,
        connector: Arc<dyn CeremonyExecutionConnectorPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            store,
            connector,
            clock,
            artifacts: None,
        }
    }

    #[must_use]
    pub fn with_artifacts(mut self, artifacts: Arc<ArtifactService>) -> Self {
        self.artifacts = Some(artifacts);
        self
    }

    pub async fn execute(
        &self,
        input: ExecuteCeremonyOperationInput,
    ) -> Result<ExecuteCeremonyOperationOutcome, DomainError> {
        self.execute_cancellable(input, ExecutionCancellation::new())
            .await
    }

    pub async fn execute_cancellable(
        &self,
        input: ExecuteCeremonyOperationInput,
        cancellation: ExecutionCancellation,
    ) -> Result<ExecuteCeremonyOperationOutcome, DomainError> {
        ensure_authority(&cancellation)?;
        let semantic_request = input.handler_request.semantic_request_bytes()?;
        let candidate = ExecutionOperation::new(
            input.handler_request.instance_id().clone(),
            input.handler_request.step_id().clone(),
            input.state_visit,
            input.state_iteration,
            input.step_iteration,
            semantic_request,
        );
        let operation = match self.store.operation(candidate.operation_id()).await? {
            Some(stored) if stored == candidate => stored,
            Some(_) => {
                return Err(DomainError::Conflict {
                    what: "execution_operation",
                })
            }
            None => candidate,
        };
        let existing_intent = self
            .store
            .intent(operation.operation_id(), &input.claim_fence)
            .await?;
        let (intent, recorded) = if let Some(intent) = existing_intent {
            if intent.operation() != &operation
                || intent.connector_id() != self.connector.connector_id()
                || intent.recovery_capability() != self.connector.recovery_capability()
                || intent.source_kind() != self.connector.source_kind()
                || intent.actor_kind() != input.actor_kind
            {
                return Err(DomainError::Conflict {
                    what: "execution_intent",
                });
            }
            (intent, RecordExecutionIntentOutcome::AlreadyRecorded)
        } else {
            let intent = ExecutionIntent::new(
                operation,
                input.claim_fence,
                self.connector.connector_id().clone(),
                self.connector.recovery_capability(),
                self.connector.source_kind(),
                input.actor_kind,
                self.clock.now(),
            )?;
            let recorded = self.store.record_intent(intent.clone()).await?;
            (intent, recorded)
        };
        if let Some(receipt) = self
            .store
            .receipt(intent.operation().operation_id())
            .await?
        {
            verify_receipt_artifacts(self.artifacts.as_deref(), &receipt).await?;
            return Ok(ExecuteCeremonyOperationOutcome::Receipt(Box::new(receipt)));
        }
        if self.reconciliation_required(&intent).await? {
            return Ok(ExecuteCeremonyOperationOutcome::ReconciliationRequired(
                intent.operation().operation_id().clone(),
            ));
        }
        if recorded != RecordExecutionIntentOutcome::RecordedFirst
            && self.connector.recovery_capability()
                == ExecutionRecoveryCapability::ReconciliationRequired
        {
            return Ok(ExecuteCeremonyOperationOutcome::ReconciliationRequired(
                intent.operation().operation_id().clone(),
            ));
        }

        let connector_outcome = self
            .connector
            .execute_cancellable(
                CeremonyExecutionRequest::new(intent.clone(), input.handler_request)?,
                cancellation,
            )
            .await?;
        let observation = match connector_outcome {
            CeremonyExecutionConnectorOutcome::Observed(observation) => *observation,
            CeremonyExecutionConnectorOutcome::ReconciliationRequired(operation_id) => {
                self.record_reconciliation_required(&intent, &operation_id)
                    .await?;
                return Ok(ExecuteCeremonyOperationOutcome::ReconciliationRequired(
                    operation_id,
                ));
            }
        };
        let receipt = execution_receipt_from_observation(
            self.store.as_ref(),
            self.connector.as_ref(),
            &intent,
            observation,
        )
        .await?;
        verify_receipt_artifacts(self.artifacts.as_deref(), &receipt).await?;
        self.store.record_receipt(receipt.clone()).await?;
        Ok(ExecuteCeremonyOperationOutcome::Receipt(Box::new(receipt)))
    }

    async fn reconciliation_required(&self, intent: &ExecutionIntent) -> Result<bool, DomainError> {
        let Some(requirement) = self
            .store
            .reconciliation_requirement(intent.operation().operation_id(), intent.claim_fence())
            .await?
        else {
            return Ok(false);
        };
        if !requirement.matches_intent(intent) {
            return Err(DomainError::InvariantViolated {
                reason: "execution reconciliation requirement does not match its intent",
            });
        }
        Ok(true)
    }

    async fn record_reconciliation_required(
        &self,
        intent: &ExecutionIntent,
        operation_id: &made_core::value_objects::ExecutionOperationId,
    ) -> Result<(), DomainError> {
        if operation_id != intent.operation().operation_id() {
            return Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement",
            });
        }
        self.store
            .record_reconciliation_requirement(ExecutionReconciliationRequirement::from_intent(
                intent,
            ))
            .await
    }
}

fn ensure_authority(cancellation: &ExecutionCancellation) -> Result<(), DomainError> {
    if cancellation.is_cancelled() {
        return Err(DomainError::InvariantViolated {
            reason: "execution authority was cancelled",
        });
    }
    Ok(())
}
