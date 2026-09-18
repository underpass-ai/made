use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{
    CeremonyExecutionConnectorPort, CeremonyExecutionRequest, ClockPort, ExecutionReceiptStorePort,
    RecordExecutionIntentOutcome,
};
use made_core::value_objects::{
    ExecutionIntent, ExecutionOperation, ExecutionReceipt, ExecutionRecoveryCapability,
};

use super::execution_receipt_from_observation::execution_receipt_from_observation;
use super::ExecuteCeremonyOperationInput;

/// Persist intent, execute or recover once, then persist an immutable receipt.
pub struct ExecuteCeremonyOperationUseCase {
    store: Arc<dyn ExecutionReceiptStorePort>,
    connector: Arc<dyn CeremonyExecutionConnectorPort>,
    clock: Arc<dyn ClockPort>,
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
        }
    }

    pub async fn execute(
        &self,
        input: ExecuteCeremonyOperationInput,
    ) -> Result<ExecutionReceipt, DomainError> {
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
            return Ok(receipt);
        }
        if recorded != RecordExecutionIntentOutcome::RecordedFirst
            && self.connector.recovery_capability()
                == ExecutionRecoveryCapability::ReconciliationRequired
        {
            return Err(DomainError::InvariantViolated {
                reason: "execution intent requires operator reconciliation before retry",
            });
        }

        let observation = self
            .connector
            .execute_or_recover(CeremonyExecutionRequest::new(
                intent.clone(),
                input.handler_request,
            )?)
            .await?;
        let receipt = execution_receipt_from_observation(
            self.store.as_ref(),
            self.connector.as_ref(),
            &intent,
            observation,
        )
        .await?;
        self.store.record_receipt(receipt.clone()).await?;
        Ok(receipt)
    }
}
