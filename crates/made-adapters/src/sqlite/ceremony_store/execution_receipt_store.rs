//! Durable recoverable-worker intents and terminal receipts.
//!
//! The operation root and its first intent are written in one immediate
//! transaction. That root is the serialization point across processes: only
//! one contender can observe it as absent and answer `RecordedFirst`.

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

use crate::engine::{Key, ReadTx, Table};
use crate::sqlite::keys::{execution_intent, execution_intent_range};

use super::{decode, encode, SqliteCeremonyStore};

fn operation(
    tx: &dyn ReadTx,
    operation_id: &ExecutionOperationId,
) -> Result<Option<ExecutionOperation>, DomainError> {
    let stored: Option<ExecutionOperation> = tx
        .get(Table::ExecutionOperations, Key::Str(operation_id.as_str()))?
        .map(|bytes| decode(&bytes, "decode execution operation"))
        .transpose()?;
    if let Some(stored) = &stored {
        stored.validate()?;
        if stored.operation_id() != operation_id {
            return Err(DomainError::InvariantViolated {
                reason: "sqlite: execution operation key does not match its value",
            });
        }
    }
    Ok(stored)
}

fn intent(
    tx: &dyn ReadTx,
    operation_id: &ExecutionOperationId,
    claim_fence: &StepClaimFence,
) -> Result<Option<ExecutionIntent>, DomainError> {
    let key = execution_intent(operation_id, claim_fence);
    let stored: Option<ExecutionIntent> = tx
        .get(Table::ExecutionIntents, Key::Bytes(&key))?
        .map(|bytes| decode(&bytes, "decode execution intent"))
        .transpose()?;
    if let Some(stored) = &stored {
        stored.validate()?;
        if stored.operation().operation_id() != operation_id || stored.claim_fence() != claim_fence
        {
            return Err(DomainError::InvariantViolated {
                reason: "sqlite: execution intent key does not match its value",
            });
        }
    }
    Ok(stored)
}

fn receipt(
    tx: &dyn ReadTx,
    operation_id: &ExecutionOperationId,
) -> Result<Option<ExecutionReceipt>, DomainError> {
    let stored: Option<ExecutionReceipt> = tx
        .get(Table::ExecutionReceipts, Key::Str(operation_id.as_str()))?
        .map(|bytes| decode(&bytes, "decode execution receipt"))
        .transpose()?;
    if let Some(stored) = &stored {
        stored.validate()?;
        if stored.operation_id() != operation_id {
            return Err(DomainError::InvariantViolated {
                reason: "sqlite: execution receipt key does not match its value",
            });
        }
    }
    Ok(stored)
}

fn reconciliation_requirement(
    tx: &dyn ReadTx,
    operation_id: &ExecutionOperationId,
    claim_fence: &StepClaimFence,
) -> Result<Option<ExecutionReconciliationRequirement>, DomainError> {
    let key = execution_intent(operation_id, claim_fence);
    let stored: Option<ExecutionReconciliationRequirement> = tx
        .get(Table::ExecutionReconciliationRequirements, Key::Bytes(&key))?
        .map(|bytes| decode(&bytes, "decode execution reconciliation requirement"))
        .transpose()?;
    if let Some(stored) = &stored {
        if stored.operation_id() != operation_id || stored.producer_claim_fence() != claim_fence {
            return Err(DomainError::InvariantViolated {
                reason: "sqlite: execution reconciliation requirement key does not match its value",
            });
        }
    }
    Ok(stored)
}

#[async_trait]
impl ExecutionReceiptStorePort for SqliteCeremonyStore {
    async fn record_intent(
        &self,
        intent_to_record: ExecutionIntent,
    ) -> Result<RecordExecutionIntentOutcome, DomainError> {
        intent_to_record.validate()?;
        self.blocking("record execution intent", move |engine| {
            let mut tx = engine.begin_write()?;
            let operation_id = intent_to_record.operation().operation_id();
            let stored_operation = operation(tx.as_ref(), operation_id)?;
            let outcome = match &stored_operation {
                Some(stored) if stored != intent_to_record.operation() => {
                    return Err(DomainError::Conflict {
                        what: "execution_operation",
                    });
                }
                Some(_) => RecordExecutionIntentOutcome::RecordedAdditional,
                None => RecordExecutionIntentOutcome::RecordedFirst,
            };

            if let Some(stored) = intent(tx.as_ref(), operation_id, intent_to_record.claim_fence())?
            {
                return if stored == intent_to_record {
                    Ok(RecordExecutionIntentOutcome::AlreadyRecorded)
                } else {
                    Err(DomainError::Conflict {
                        what: "execution_intent",
                    })
                };
            }

            if stored_operation.is_none() {
                tx.insert(
                    Table::ExecutionOperations,
                    Key::Str(operation_id.as_str()),
                    &encode(intent_to_record.operation(), "encode execution operation")?,
                )?;
            }
            let intent_key = execution_intent(operation_id, intent_to_record.claim_fence());
            tx.insert(
                Table::ExecutionIntents,
                Key::Bytes(&intent_key),
                &encode(&intent_to_record, "encode execution intent")?,
            )?;
            tx.commit()?;
            Ok(outcome)
        })
        .await
    }

    async fn intent(
        &self,
        operation_id: &ExecutionOperationId,
        claim_fence: &StepClaimFence,
    ) -> Result<Option<ExecutionIntent>, DomainError> {
        let operation_id = operation_id.clone();
        let claim_fence = claim_fence.clone();
        self.blocking("read execution intent", move |engine| {
            let tx = engine.begin_read()?;
            intent(tx.as_ref(), &operation_id, &claim_fence)
        })
        .await
    }

    async fn receipt(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<ExecutionReceipt>, DomainError> {
        let operation_id = operation_id.clone();
        self.blocking("read execution receipt", move |engine| {
            let tx = engine.begin_read()?;
            receipt(tx.as_ref(), &operation_id)
        })
        .await
    }

    async fn record_reconciliation_requirement(
        &self,
        requirement: ExecutionReconciliationRequirement,
    ) -> Result<(), DomainError> {
        self.blocking(
            "record execution reconciliation requirement",
            move |engine| {
                let mut tx = engine.begin_write()?;
                let operation_id = requirement.operation_id();
                let claim_fence = requirement.producer_claim_fence();
                let Some(producer_intent) = intent(tx.as_ref(), operation_id, claim_fence)? else {
                    return Err(DomainError::NotFound {
                        what: "execution_intent",
                    });
                };
                if !requirement.matches_intent(&producer_intent) {
                    return Err(DomainError::Conflict {
                        what: "execution_reconciliation_requirement",
                    });
                }
                if let Some(stored) =
                    reconciliation_requirement(tx.as_ref(), operation_id, claim_fence)?
                {
                    return if stored == requirement {
                        Ok(())
                    } else {
                        Err(DomainError::Conflict {
                            what: "execution_reconciliation_requirement",
                        })
                    };
                }
                let key = execution_intent(operation_id, claim_fence);
                tx.insert(
                    Table::ExecutionReconciliationRequirements,
                    Key::Bytes(&key),
                    &encode(&requirement, "encode execution reconciliation requirement")?,
                )?;
                tx.commit()
            },
        )
        .await
    }

    async fn reconciliation_requirement(
        &self,
        operation_id: &ExecutionOperationId,
        claim_fence: &StepClaimFence,
    ) -> Result<Option<ExecutionReconciliationRequirement>, DomainError> {
        let operation_id = operation_id.clone();
        let claim_fence = claim_fence.clone();
        self.blocking("read execution reconciliation requirement", move |engine| {
            let tx = engine.begin_read()?;
            reconciliation_requirement(tx.as_ref(), &operation_id, &claim_fence)
        })
        .await
    }

    async fn intents(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Vec<ExecutionIntent>, DomainError> {
        let operation_id = operation_id.clone();
        self.blocking("read execution intents", move |engine| {
            let tx = engine.begin_read()?;
            let (start, end) = execution_intent_range(&operation_id);
            tx.scan_bytes_range(Table::ExecutionIntents, &start, &end)?
                .into_iter()
                .map(|(_, bytes)| {
                    let stored: ExecutionIntent = decode(&bytes, "decode execution intent")?;
                    stored.validate()?;
                    if stored.operation().operation_id() != &operation_id {
                        return Err(DomainError::InvariantViolated {
                            reason: "sqlite: execution intent range contains another operation",
                        });
                    }
                    Ok(stored)
                })
                .collect()
        })
        .await
    }

    async fn operation(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<ExecutionOperation>, DomainError> {
        let operation_id = operation_id.clone();
        self.blocking("read execution operation", move |engine| {
            let tx = engine.begin_read()?;
            operation(tx.as_ref(), &operation_id)
        })
        .await
    }

    async fn record_receipt(
        &self,
        receipt_to_record: ExecutionReceipt,
    ) -> Result<RecordExecutionReceiptOutcome, DomainError> {
        receipt_to_record.validate()?;
        self.blocking("record execution receipt", move |engine| {
            let mut tx = engine.begin_write()?;
            let operation_id = receipt_to_record.operation_id();
            let Some(stored_operation) = operation(tx.as_ref(), operation_id)? else {
                return Err(DomainError::NotFound {
                    what: "execution_operation",
                });
            };
            let Some(producer_intent) = intent(
                tx.as_ref(),
                operation_id,
                receipt_to_record.producer_claim_fence(),
            )?
            else {
                return Err(DomainError::InvariantViolated {
                    reason: "execution receipt does not match a recorded intent",
                });
            };
            if stored_operation.request_digest() != receipt_to_record.request_digest()
                || producer_intent.connector_id() != receipt_to_record.connector_id()
                || producer_intent.recovery_capability() != receipt_to_record.recovery_capability()
                || producer_intent.source_kind() != receipt_to_record.source_kind()
            {
                return Err(DomainError::InvariantViolated {
                    reason: "execution receipt does not match its producer intent contract",
                });
            }
            if let Some(stored) = receipt(tx.as_ref(), operation_id)? {
                return if stored == receipt_to_record {
                    Ok(RecordExecutionReceiptOutcome::AlreadyRecorded)
                } else {
                    Err(DomainError::Conflict {
                        what: "execution_receipt",
                    })
                };
            }
            tx.insert(
                Table::ExecutionReceipts,
                Key::Str(operation_id.as_str()),
                &encode(&receipt_to_record, "encode execution receipt")?,
            )?;
            tx.commit()?;
            Ok(RecordExecutionReceiptOutcome::Recorded)
        })
        .await
    }

    async fn recoverable(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
        limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryPage, DomainError> {
        let after = after.cloned();
        self.blocking("scan recoverable executions", move |engine| {
            let tx = engine.begin_read()?;
            let wanted = usize::from(limit.get());
            let rows = tx.scan_str_after(
                Table::ExecutionOperations,
                after.as_ref().map(ExecutionRecoveryCursor::as_str),
                wanted + 1,
            )?;
            let has_more = rows.len() > wanted;
            let operations = rows
                .into_iter()
                .take(wanted)
                .map(|(key, bytes)| {
                    let operation: ExecutionOperation =
                        decode(&bytes, "decode recoverable execution operation")?;
                    if operation.operation_id().as_str() != key {
                        return Err(DomainError::InvariantViolated {
                            reason: "sqlite: execution operation key does not match its value",
                        });
                    }
                    Ok(operation)
                })
                .collect::<Result<Vec<_>, DomainError>>()?;
            let next_cursor = if has_more {
                operations
                    .last()
                    .map(|operation| {
                        ExecutionRecoveryCursor::new(operation.operation_id().as_str())
                    })
                    .transpose()?
            } else {
                None
            };
            Ok(ExecutionRecoveryPage::new(operations, next_cursor))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use made_core::ports::{
        ExecutionReceiptStorePort, RecordExecutionIntentOutcome, RecordExecutionReceiptOutcome,
    };
    use made_core::value_objects::{
        ArtifactSourceKind, AuditActorKind, CeremonyId, ExecutionConnectorId, ExecutionIntent,
        ExecutionOperation, ExecutionReceipt, ExecutionReconciliationRequirement,
        ExecutionRecoveryCapability, ExecutionRecoveryPageLimit, ExecutionRequestBytes,
        StateIteration, StateVisit, StepClaimFence, StepId, StepIteration, StepOutput, StepResult,
    };
    use time::OffsetDateTime;

    use super::SqliteCeremonyStore;

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
    async fn two_handles_atomically_choose_one_first_intent_and_reopen_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("receipts.sqlite3");
        let left = SqliteCeremonyStore::open(&path).unwrap();
        let right = SqliteCeremonyStore::open(&path).unwrap();
        let operation = operation("work", b"same semantic request");
        let first = intent(operation.clone(), fence('1'));
        let reclaimed = intent(operation.clone(), fence('2'));

        let (left_outcome, right_outcome) =
            tokio::join!(left.record_intent(first), right.record_intent(reclaimed));
        let outcomes = [left_outcome.unwrap(), right_outcome.unwrap()];
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
        drop((left, right));

        let reopened = SqliteCeremonyStore::open(path).unwrap();
        let page = reopened
            .recoverable(None, ExecutionRecoveryPageLimit::new(1).unwrap())
            .await
            .unwrap();
        assert_eq!(page.operations(), std::slice::from_ref(&operation));
        assert!(reopened
            .intent(operation.operation_id(), &fence('1'))
            .await
            .unwrap()
            .is_some());
        assert!(reopened
            .intent(operation.operation_id(), &fence('2'))
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn connector_reported_ambiguity_is_idempotent_and_survives_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("reconciliation.sqlite3");
        let left = SqliteCeremonyStore::open(&path).unwrap();
        let right = SqliteCeremonyStore::open(&path).unwrap();
        let intent = intent(operation("ambiguous", b"request"), fence('4'));
        left.record_intent(intent.clone()).await.unwrap();
        let requirement = ExecutionReconciliationRequirement::from_intent(&intent);

        let (left_result, right_result) = tokio::join!(
            left.record_reconciliation_requirement(requirement.clone()),
            right.record_reconciliation_requirement(requirement.clone()),
        );
        left_result.unwrap();
        right_result.unwrap();
        drop((left, right));

        let reopened = SqliteCeremonyStore::open(path).unwrap();
        assert_eq!(
            reopened
                .reconciliation_requirement(intent.operation().operation_id(), intent.claim_fence())
                .await
                .unwrap(),
            Some(requirement)
        );
    }

    #[tokio::test]
    async fn receipt_contract_mismatches_do_not_mutate_the_durable_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("receipts.sqlite3");
        let store = SqliteCeremonyStore::open(&path).unwrap();
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
            store.record_receipt(accepted.clone()).await.unwrap(),
            RecordExecutionReceiptOutcome::Recorded
        );
        drop(store);
        let reopened = SqliteCeremonyStore::open(path).unwrap();
        assert_eq!(
            reopened.receipt(operation.operation_id()).await.unwrap(),
            Some(accepted)
        );
    }
}
