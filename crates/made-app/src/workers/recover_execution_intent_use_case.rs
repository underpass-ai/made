use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{CeremonyExecutionConnectorPort, ExecutionReceiptStorePort};
use made_core::value_objects::{ExecutionIntent, ExecutionRecoveryCapability};

use super::execution_receipt_from_observation::execution_receipt_from_observation;
use super::RecoverExecutionIntentOutcome;

/// Recover one persisted intent without relying on process-local request state.
pub struct RecoverExecutionIntentUseCase {
    store: Arc<dyn ExecutionReceiptStorePort>,
    connector: Arc<dyn CeremonyExecutionConnectorPort>,
}

impl std::fmt::Debug for RecoverExecutionIntentUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoverExecutionIntentUseCase")
            .field("connector_id", &self.connector.connector_id())
            .finish_non_exhaustive()
    }
}

impl RecoverExecutionIntentUseCase {
    #[must_use]
    pub fn new(
        store: Arc<dyn ExecutionReceiptStorePort>,
        connector: Arc<dyn CeremonyExecutionConnectorPort>,
    ) -> Self {
        Self { store, connector }
    }

    pub async fn execute(
        &self,
        intent: &ExecutionIntent,
    ) -> Result<RecoverExecutionIntentOutcome, DomainError> {
        if let Some(receipt) = self
            .store
            .receipt(intent.operation().operation_id())
            .await?
        {
            return Ok(RecoverExecutionIntentOutcome::Receipt(Box::new(receipt)));
        }
        if intent.connector_id() != self.connector.connector_id()
            || intent.recovery_capability() != self.connector.recovery_capability()
            || intent.source_kind() != self.connector.source_kind()
        {
            return Err(DomainError::Conflict {
                what: "execution_connector_contract",
            });
        }
        if intent.recovery_capability() == ExecutionRecoveryCapability::ReconciliationRequired {
            return Ok(RecoverExecutionIntentOutcome::ReconciliationRequired(
                intent.operation().operation_id().clone(),
            ));
        }
        let observation = self.connector.recover_intent(intent).await?;
        let receipt = execution_receipt_from_observation(
            self.store.as_ref(),
            self.connector.as_ref(),
            intent,
            observation,
        )
        .await?;
        self.store.record_receipt(receipt.clone()).await?;
        Ok(RecoverExecutionIntentOutcome::Receipt(Box::new(receipt)))
    }
}
