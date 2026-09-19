use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    ExecutionReceiptStorePort, ExecutionRecoveryPage, RecordExecutionIntentOutcome,
    RecordExecutionReceiptOutcome,
};
use made_core::value_objects::{
    ExecutionIntent, ExecutionOperation, ExecutionOperationId, ExecutionReceipt,
    ExecutionReconciliationRequirement, ExecutionRecoveryCursor, ExecutionRecoveryPageLimit,
    StepClaimFence,
};
use tokio::sync::Mutex;

use super::execution_receipt_store_state::ExecutionReceiptStoreState;

/// Ephemeral receipt store with the same atomic root semantics as durable adapters.
#[derive(Debug, Default)]
pub struct InMemoryExecutionReceiptStore {
    state: Mutex<ExecutionReceiptStoreState>,
}

impl InMemoryExecutionReceiptStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ExecutionReceiptStorePort for InMemoryExecutionReceiptStore {
    async fn record_intent(
        &self,
        intent: ExecutionIntent,
    ) -> Result<RecordExecutionIntentOutcome, DomainError> {
        intent.validate()?;
        let mut state = self.state.lock().await;
        let operation_id = intent.operation().operation_id().as_str().to_owned();
        let key = (
            operation_id.clone(),
            intent.claim_fence().as_str().to_owned(),
        );

        if let Some(stored) = state.intents.get(&key) {
            return if stored == &intent {
                Ok(RecordExecutionIntentOutcome::AlreadyRecorded)
            } else {
                Err(DomainError::Conflict {
                    what: "execution_intent",
                })
            };
        }

        let outcome = match state.operations.get(&operation_id) {
            Some(stored) if stored != intent.operation() => {
                return Err(DomainError::Conflict {
                    what: "execution_operation",
                });
            }
            Some(_) => RecordExecutionIntentOutcome::RecordedAdditional,
            None => {
                state
                    .operations
                    .insert(operation_id.clone(), intent.operation().clone());
                RecordExecutionIntentOutcome::RecordedFirst
            }
        };
        state.intents.insert(key, intent);
        Ok(outcome)
    }

    async fn intent(
        &self,
        operation_id: &ExecutionOperationId,
        claim_fence: &StepClaimFence,
    ) -> Result<Option<ExecutionIntent>, DomainError> {
        Ok(self
            .state
            .lock()
            .await
            .intents
            .get(&(
                operation_id.as_str().to_owned(),
                claim_fence.as_str().to_owned(),
            ))
            .cloned())
    }

    async fn receipt(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<ExecutionReceipt>, DomainError> {
        Ok(self
            .state
            .lock()
            .await
            .receipts
            .get(operation_id.as_str())
            .cloned())
    }

    async fn record_reconciliation_requirement(
        &self,
        requirement: ExecutionReconciliationRequirement,
    ) -> Result<(), DomainError> {
        let mut state = self.state.lock().await;
        let key = (
            requirement.operation_id().as_str().to_owned(),
            requirement.producer_claim_fence().as_str().to_owned(),
        );
        let Some(intent) = state.intents.get(&key) else {
            return Err(DomainError::NotFound {
                what: "execution_intent",
            });
        };
        if !requirement.matches_intent(intent) {
            return Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement",
            });
        }
        match state.reconciliation_requirements.get(&key) {
            Some(stored) if stored == &requirement => Ok(()),
            Some(_) => Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement",
            }),
            None => {
                state.reconciliation_requirements.insert(key, requirement);
                Ok(())
            }
        }
    }

    async fn reconciliation_requirement(
        &self,
        operation_id: &ExecutionOperationId,
        claim_fence: &StepClaimFence,
    ) -> Result<Option<ExecutionReconciliationRequirement>, DomainError> {
        Ok(self
            .state
            .lock()
            .await
            .reconciliation_requirements
            .get(&(
                operation_id.as_str().to_owned(),
                claim_fence.as_str().to_owned(),
            ))
            .cloned())
    }

    async fn intents(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Vec<ExecutionIntent>, DomainError> {
        let operation_id = operation_id.as_str();
        Ok(self
            .state
            .lock()
            .await
            .intents
            .iter()
            .filter(|((candidate, _), _)| candidate == operation_id)
            .map(|(_, intent)| intent.clone())
            .collect())
    }

    async fn operation(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<ExecutionOperation>, DomainError> {
        Ok(self
            .state
            .lock()
            .await
            .operations
            .get(operation_id.as_str())
            .cloned())
    }

    async fn record_receipt(
        &self,
        receipt: ExecutionReceipt,
    ) -> Result<RecordExecutionReceiptOutcome, DomainError> {
        receipt.validate()?;
        let mut state = self.state.lock().await;
        let operation_id = receipt.operation_id().as_str().to_owned();
        let Some(operation) = state.operations.get(&operation_id) else {
            return Err(DomainError::NotFound {
                what: "execution_operation",
            });
        };
        let producer_key = (
            operation_id.clone(),
            receipt.producer_claim_fence().as_str().to_owned(),
        );
        let Some(producer_intent) = state.intents.get(&producer_key) else {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt does not match a recorded intent",
            });
        };
        if operation.request_digest() != receipt.request_digest()
            || producer_intent.connector_id() != receipt.connector_id()
            || producer_intent.recovery_capability() != receipt.recovery_capability()
            || producer_intent.source_kind() != receipt.source_kind()
        {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt does not match its producer intent contract",
            });
        }
        match state.receipts.get(&operation_id) {
            Some(stored) if stored == &receipt => {
                Ok(RecordExecutionReceiptOutcome::AlreadyRecorded)
            }
            Some(_) => Err(DomainError::Conflict {
                what: "execution_receipt",
            }),
            None => {
                state.receipts.insert(operation_id, receipt);
                Ok(RecordExecutionReceiptOutcome::Recorded)
            }
        }
    }

    async fn recoverable(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
        limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryPage, DomainError> {
        let state = self.state.lock().await;
        let mut matching = state
            .operations
            .values()
            .filter(|operation| {
                after.is_none_or(|cursor| operation.operation_id().as_str() > cursor.as_str())
            })
            .take(usize::from(limit.get()) + 1)
            .cloned()
            .collect::<Vec<_>>();
        let has_more = matching.len() > usize::from(limit.get());
        matching.truncate(usize::from(limit.get()));
        let next_cursor = has_more
            .then(|| matching.last())
            .flatten()
            .map(|operation| ExecutionRecoveryCursor::new(operation.operation_id().as_str()))
            .transpose()?;
        Ok(ExecutionRecoveryPage::new(matching, next_cursor))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use made_core::ports::{
        ExecutionReceiptStorePort, RecordExecutionIntentOutcome, RecordExecutionReceiptOutcome,
    };
    use made_core::value_objects::{
        ArtifactSourceKind, AuditActorKind, CeremonyId, ExecutionConnectorId, ExecutionIntent,
        ExecutionOperation, ExecutionReceipt, ExecutionRecoveryCapability,
        ExecutionRecoveryPageLimit, ExecutionRequestBytes, StateIteration, StateVisit,
        StepClaimFence, StepId, StepIteration, StepOutput, StepResult,
    };
    use time::OffsetDateTime;

    use super::InMemoryExecutionReceiptStore;

    fn operation(step: &str, request: &[u8]) -> ExecutionOperation {
        ExecutionOperation::new(
            CeremonyId::new("ceremony").unwrap(),
            StepId::new(step).unwrap(),
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
            ExecutionRequestBytes::new(request.to_vec()).unwrap(),
        )
    }

    fn fence(digit: char) -> StepClaimFence {
        StepClaimFence::new(digit.to_string().repeat(64)).unwrap()
    }

    fn intent(operation: ExecutionOperation, fence: StepClaimFence) -> ExecutionIntent {
        ExecutionIntent::new(
            operation,
            fence,
            ExecutionConnectorId::new("test.no-op").unwrap(),
            ExecutionRecoveryCapability::IdempotentByOperationId,
            ArtifactSourceKind::NoOp,
            AuditActorKind::Engine,
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    fn receipt(
        operation: &ExecutionOperation,
        accepted_fence: StepClaimFence,
        connector: &str,
        capability: ExecutionRecoveryCapability,
        source_kind: ArtifactSourceKind,
    ) -> ExecutionReceipt {
        ExecutionReceipt::new(
            operation.operation_id().clone(),
            operation.request_digest().clone(),
            accepted_fence,
            ExecutionConnectorId::new(connector).unwrap(),
            None,
            capability,
            source_kind,
            StepResult::completed(StepOutput::empty()).unwrap(),
            Vec::new(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn one_operation_root_is_first_across_competing_fences() {
        let store = Arc::new(InMemoryExecutionReceiptStore::new());
        let operation = operation("work", b"same semantic request");
        let first = intent(operation.clone(), fence('1'));
        let reclaimed = intent(operation, fence('2'));

        let (left, right) =
            tokio::join!(store.record_intent(first), store.record_intent(reclaimed));
        let outcomes = [left.unwrap(), right.unwrap()];
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == RecordExecutionIntentOutcome::RecordedFirst)
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == RecordExecutionIntentOutcome::RecordedAdditional)
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn reclaim_cannot_change_the_sealed_semantic_request() {
        let store = InMemoryExecutionReceiptStore::new();
        let original = operation("work", b"original");
        store
            .record_intent(intent(original.clone(), fence('1')))
            .await
            .unwrap();

        let changed = operation("work", b"changed");
        assert_eq!(original.operation_id(), changed.operation_id());
        assert_ne!(original.request_digest(), changed.request_digest());
        assert!(store
            .record_intent(intent(changed, fence('2')))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn receipt_is_immutable_and_a_foreign_fence_is_rejected() {
        let store = InMemoryExecutionReceiptStore::new();
        let operation = operation("work", b"request");
        let accepted_fence = fence('1');
        store
            .record_intent(intent(operation.clone(), accepted_fence.clone()))
            .await
            .unwrap();

        let receipt = ExecutionReceipt::new(
            operation.operation_id().clone(),
            operation.request_digest().clone(),
            accepted_fence,
            ExecutionConnectorId::new("test.no-op").unwrap(),
            None,
            ExecutionRecoveryCapability::IdempotentByOperationId,
            ArtifactSourceKind::NoOp,
            StepResult::completed(StepOutput::empty()).unwrap(),
            Vec::new(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        assert_eq!(
            store.record_receipt(receipt.clone()).await.unwrap(),
            RecordExecutionReceiptOutcome::Recorded
        );
        assert_eq!(
            store.record_receipt(receipt).await.unwrap(),
            RecordExecutionReceiptOutcome::AlreadyRecorded
        );

        let foreign = ExecutionReceipt::new(
            operation.operation_id().clone(),
            operation.request_digest().clone(),
            fence('2'),
            ExecutionConnectorId::new("test.no-op").unwrap(),
            None,
            ExecutionRecoveryCapability::IdempotentByOperationId,
            ArtifactSourceKind::NoOp,
            StepResult::completed(StepOutput::empty()).unwrap(),
            Vec::new(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        assert!(store.record_receipt(foreign).await.is_err());
    }

    #[tokio::test]
    async fn mismatched_receipt_contracts_are_rejected_without_mutation() {
        let store = InMemoryExecutionReceiptStore::new();
        let operation = operation("work", b"request");
        let accepted_fence = fence('1');
        store
            .record_intent(intent(operation.clone(), accepted_fence.clone()))
            .await
            .unwrap();

        for mismatched in [
            receipt(
                &operation,
                accepted_fence.clone(),
                "other.connector",
                ExecutionRecoveryCapability::IdempotentByOperationId,
                ArtifactSourceKind::NoOp,
            ),
            receipt(
                &operation,
                accepted_fence.clone(),
                "test.no-op",
                ExecutionRecoveryCapability::ReconciliationRequired,
                ArtifactSourceKind::NoOp,
            ),
            receipt(
                &operation,
                accepted_fence.clone(),
                "test.no-op",
                ExecutionRecoveryCapability::IdempotentByOperationId,
                ArtifactSourceKind::Fixture,
            ),
        ] {
            assert!(store.record_receipt(mismatched).await.is_err());
            assert_eq!(store.receipt(operation.operation_id()).await.unwrap(), None);
        }

        let accepted = receipt(
            &operation,
            accepted_fence,
            "test.no-op",
            ExecutionRecoveryCapability::IdempotentByOperationId,
            ArtifactSourceKind::NoOp,
        );
        assert_eq!(
            store.record_receipt(accepted).await.unwrap(),
            RecordExecutionReceiptOutcome::Recorded
        );
    }

    #[tokio::test]
    async fn recovery_keeps_operations_that_already_have_a_receipt() {
        let store = InMemoryExecutionReceiptStore::new();
        let operation = operation("work", b"request");
        let accepted_fence = fence('1');
        store
            .record_intent(intent(operation.clone(), accepted_fence.clone()))
            .await
            .unwrap();
        store
            .record_receipt(
                ExecutionReceipt::new(
                    operation.operation_id().clone(),
                    operation.request_digest().clone(),
                    accepted_fence,
                    ExecutionConnectorId::new("test.no-op").unwrap(),
                    None,
                    ExecutionRecoveryCapability::IdempotentByOperationId,
                    ArtifactSourceKind::NoOp,
                    StepResult::completed(StepOutput::empty()).unwrap(),
                    Vec::new(),
                    OffsetDateTime::UNIX_EPOCH,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        let page = store
            .recoverable(None, ExecutionRecoveryPageLimit::new(1).unwrap())
            .await
            .unwrap();
        assert_eq!(page.operations(), &[operation]);
    }
}
