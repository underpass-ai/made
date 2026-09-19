use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::ExecutionCancellation;
use made_core::value_objects::ExecutionReceiptLinkKind;

use super::{
    CompleteExecutionReceiptInput, CompleteExecutionReceiptUseCase, ExecuteCeremonyOperationInput,
    ExecuteCeremonyOperationOutcome, ExecuteCeremonyOperationUseCase, ExecutionRecoveryItem,
    RecoverExecutionIntentOutcome, RecoverExecutionIntentUseCase, RecoverableCeremonyWorkerOutcome,
    RecoverableCeremonyWorkerPort,
};

/// Runs an accepted claim through receipt persistence and fenced completion.
pub struct RecoverableCeremonyWorker {
    execute_operation: Arc<ExecuteCeremonyOperationUseCase>,
    recover_intent: Arc<RecoverExecutionIntentUseCase>,
    complete_receipt: Arc<CompleteExecutionReceiptUseCase>,
}

impl std::fmt::Debug for RecoverableCeremonyWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoverableCeremonyWorker")
            .finish_non_exhaustive()
    }
}

impl RecoverableCeremonyWorker {
    #[must_use]
    pub const fn new(
        execute_operation: Arc<ExecuteCeremonyOperationUseCase>,
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
        input: ExecuteCeremonyOperationInput,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        self.execute_claim_cancellable(input, ExecutionCancellation::new())
            .await
    }

    pub async fn execute_claim_cancellable(
        &self,
        input: ExecuteCeremonyOperationInput,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        let ceremony_id = input.handler_request.instance_id().clone();
        let step_id = input.handler_request.step_id().clone();
        let claim_fence = input.claim_fence.clone();
        let actor_kind = input.actor_kind;
        let receipt = match self
            .execute_operation
            .execute_cancellable(input, cancellation)
            .await?
        {
            ExecuteCeremonyOperationOutcome::Receipt(receipt) => *receipt,
            ExecuteCeremonyOperationOutcome::ReconciliationRequired(operation_id) => {
                return Ok(RecoverableCeremonyWorkerOutcome::ReconciliationRequired(
                    operation_id,
                ));
            }
        };
        let instance = self
            .complete_receipt
            .execute(CompleteExecutionReceiptInput {
                ceremony_id,
                step_id,
                operation_id: receipt.operation_id().clone(),
                claim_fence,
                link_kind: ExecutionReceiptLinkKind::Direct,
                actor_kind,
            })
            .await?;
        Ok(RecoverableCeremonyWorkerOutcome::Completed {
            receipt: Box::new(receipt),
            instance: Box::new(instance),
        })
    }

    pub async fn recover(
        &self,
        item: ExecutionRecoveryItem,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
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
                    return Ok(RecoverableCeremonyWorkerOutcome::ReconciliationRequired(
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
                ceremony_id: operation.ceremony_id().clone(),
                step_id: operation.step_id().clone(),
                operation_id: operation.operation_id().clone(),
                claim_fence: applied_fence,
                link_kind,
                actor_kind,
            })
            .await?;
        Ok(RecoverableCeremonyWorkerOutcome::Completed {
            receipt: Box::new(receipt),
            instance: Box::new(instance),
        })
    }
}

#[async_trait]
impl RecoverableCeremonyWorkerPort for RecoverableCeremonyWorker {
    async fn execute_claim(
        &self,
        input: ExecuteCeremonyOperationInput,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        RecoverableCeremonyWorker::execute_claim(self, input).await
    }

    async fn execute_claim_cancellable(
        &self,
        input: ExecuteCeremonyOperationInput,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        RecoverableCeremonyWorker::execute_claim_cancellable(self, input, cancellation).await
    }

    async fn recover(
        &self,
        item: ExecutionRecoveryItem,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        RecoverableCeremonyWorker::recover(self, item).await
    }

    async fn recover_cancellable(
        &self,
        item: ExecutionRecoveryItem,
        cancellation: ExecutionCancellation,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        Box::pin(cancellation.run(RecoverableCeremonyWorker::recover(self, item)))
            .await
            .ok_or(DomainError::InvariantViolated {
                reason: "worker recovery authority was cancelled",
            })?
    }
}
