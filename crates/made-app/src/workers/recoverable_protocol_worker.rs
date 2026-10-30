use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::ExecutionCancellation;
use made_core::value_objects::ExecutionReceiptLinkKind;

use super::{
    CompleteExecutionReceiptInput, CompleteExecutionReceiptUseCase, ExecuteProtocolOperationInput,
    ExecuteProtocolOperationOutcome, ExecuteProtocolOperationUseCase, ExecutionRecoveryItem,
    RecoverExecutionIntentOutcome, RecoverExecutionIntentUseCase, RecoverableProtocolWorkerOutcome,
    RecoverableProtocolWorkerPort,
};

/// Runs an accepted claim through receipt persistence and fenced completion.
pub struct RecoverableProtocolWorker {
    execute_operation: Arc<ExecuteProtocolOperationUseCase>,
    recover_intent: Arc<RecoverExecutionIntentUseCase>,
    complete_receipt: Arc<CompleteExecutionReceiptUseCase>,
}

impl std::fmt::Debug for RecoverableProtocolWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoverableProtocolWorker")
            .finish_non_exhaustive()
    }
}

impl RecoverableProtocolWorker {
    #[must_use]
    pub const fn new(
        execute_operation: Arc<ExecuteProtocolOperationUseCase>,
        recover_intent: Arc<RecoverExecutionIntentUseCase>,
        complete_receipt: Arc<CompleteExecutionReceiptUseCase>,
    ) -> Self {
        Self {
            execute_operation,
            recover_intent,
            complete_receipt,
        }
    }

    pub async fn execute_claim(
        &self,
        input: ExecuteProtocolOperationInput,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        self.execute_claim_cancellable(input, ExecutionCancellation::new())
            .await
    }

    pub async fn execute_claim_cancellable(
        &self,
        input: ExecuteProtocolOperationInput,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        let protocol_id = input.handler_request.instance_id().clone();
        let step_id = input.handler_request.step_id().clone();
        let claim_fence = input.claim_fence.clone();
        let actor_kind = input.actor_kind;
        let receipt = match self
            .execute_operation
            .execute_cancellable(input, cancellation)
            .await?
        {
            ExecuteProtocolOperationOutcome::Receipt(receipt) => *receipt,
            ExecuteProtocolOperationOutcome::ReconciliationRequired(operation_id) => {
                return Ok(RecoverableProtocolWorkerOutcome::ReconciliationRequired(
                    operation_id,
                ));
            }
        };
        let instance = self
            .complete_receipt
            .execute(CompleteExecutionReceiptInput {
                protocol_id,
                step_id,
                operation_id: receipt.operation_id().clone(),
                claim_fence,
                link_kind: ExecutionReceiptLinkKind::Direct,
                actor_kind,
            })
            .await?;
        Ok(RecoverableProtocolWorkerOutcome::Completed {
            receipt: Box::new(receipt),
            instance: Box::new(instance),
        })
    }

    pub async fn recover(
        &self,
        item: ExecutionRecoveryItem,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        let (operation, intents, receipt, current_claim_fence) = item.into_parts();
        let intent = current_claim_fence
            .as_ref()
            .and_then(|fence| intents.iter().find(|intent| intent.claim_fence() == fence))
            .or_else(|| intents.iter().min_by_key(|intent| intent.recorded_at()))
            .ok_or(DomainError::InvariantViolated {
                reason: "execution recovery item has no durable intent",
            })?;
        let receipt = match receipt {
            Some(receipt) => receipt,
            None => match self.recover_intent.execute(intent).await? {
                RecoverExecutionIntentOutcome::Receipt(receipt) => *receipt,
                RecoverExecutionIntentOutcome::ReconciliationRequired(operation_id) => {
                    return Ok(RecoverableProtocolWorkerOutcome::ReconciliationRequired(
                        operation_id,
                    ));
                }
            },
        };
        let applied_fence =
            current_claim_fence.unwrap_or_else(|| receipt.producer_claim_fence().clone());
        let link_kind = if &applied_fence == receipt.producer_claim_fence() {
            ExecutionReceiptLinkKind::Direct
        } else {
            ExecutionReceiptLinkKind::Adopted
        };
        let actor_kind = intents
            .iter()
            .find(|candidate| candidate.claim_fence() == &applied_fence)
            .unwrap_or(intent)
            .actor_kind();
        let instance = self
            .complete_receipt
            .execute(CompleteExecutionReceiptInput {
                protocol_id: operation.protocol_id().clone(),
                step_id: operation.step_id().clone(),
                operation_id: operation.operation_id().clone(),
                claim_fence: applied_fence,
                link_kind,
                actor_kind,
            })
            .await?;
        Ok(RecoverableProtocolWorkerOutcome::Completed {
            receipt: Box::new(receipt),
            instance: Box::new(instance),
        })
    }
}

#[async_trait]
impl RecoverableProtocolWorkerPort for RecoverableProtocolWorker {
    async fn execute_claim(
        &self,
        input: ExecuteProtocolOperationInput,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        RecoverableProtocolWorker::execute_claim(self, input).await
    }

    async fn execute_claim_cancellable(
        &self,
        input: ExecuteProtocolOperationInput,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        RecoverableProtocolWorker::execute_claim_cancellable(self, input, cancellation).await
    }

    async fn recover(
        &self,
        item: ExecutionRecoveryItem,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        RecoverableProtocolWorker::recover(self, item).await
    }

    async fn recover_cancellable(
        &self,
        item: ExecutionRecoveryItem,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableProtocolWorkerOutcome, DomainError> {
        Box::pin(cancellation.run(RecoverableProtocolWorker::recover(self, item)))
            .await
            .ok_or(DomainError::InvariantViolated {
                reason: "worker recovery authority was cancelled",
            })?
    }
}
