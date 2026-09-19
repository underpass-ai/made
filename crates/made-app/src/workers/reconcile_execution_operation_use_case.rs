use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{ExecutionReceiptStorePort, RecordExecutionReceiptOutcome};
use made_core::value_objects::ExecutionReceipt;

use super::{ReconcileExecutionOperationInput, ReconcileExecutionOperationOutcome};
use crate::artifacts::ArtifactService;

/// Resolve one ambiguous effect from explicit, artifact-backed operator evidence.
/// This path never invokes an execution connector or removes the original intent.
pub struct ReconcileExecutionOperationUseCase {
    store: Arc<dyn ExecutionReceiptStorePort>,
    artifacts: Arc<ArtifactService>,
}

impl std::fmt::Debug for ReconcileExecutionOperationUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReconcileExecutionOperationUseCase")
            .finish_non_exhaustive()
    }
}

impl ReconcileExecutionOperationUseCase {
    #[must_use]
    pub fn new(store: Arc<dyn ExecutionReceiptStorePort>, artifacts: Arc<ArtifactService>) -> Self {
        Self { store, artifacts }
    }

    pub async fn execute(
        &self,
        input: ReconcileExecutionOperationInput,
    ) -> Result<ReconcileExecutionOperationOutcome, DomainError> {
        if input.evidence.is_empty() {
            return Err(DomainError::EmptyField {
                field: "execution_reconciliation.evidence",
            });
        }
        let intent = self
            .store
            .intent(&input.operation_id, &input.producer_claim_fence)
            .await?
            .ok_or(DomainError::NotFound {
                what: "execution_intent",
            })?;
        if intent.operation().operation_id() != &input.operation_id
            || intent.operation().request_digest() != &input.request_digest
            || intent.claim_fence() != &input.producer_claim_fence
            || intent.connector_id() != &input.connector_id
        {
            return Err(DomainError::Conflict {
                what: "execution_reconciliation",
            });
        }
        let requirement = self
            .store
            .reconciliation_requirement(&input.operation_id, &input.producer_claim_fence)
            .await?
            .ok_or(DomainError::Conflict {
                what: "execution_reconciliation_requirement",
            })?;
        if !requirement.matches_intent(&intent)
            || requirement.operation_id() != &input.operation_id
            || requirement.request_digest() != &input.request_digest
            || requirement.producer_claim_fence() != &input.producer_claim_fence
            || requirement.connector_id() != &input.connector_id
        {
            return Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement",
            });
        }

        let receipt = ExecutionReceipt::new(
            input.operation_id,
            input.request_digest,
            input.producer_claim_fence,
            input.connector_id,
            input.external_operation_id,
            intent.recovery_capability(),
            intent.source_kind(),
            input.result,
            input.evidence,
            input.observed_at,
        )?;
        if let Some(existing) = self.store.receipt(receipt.operation_id()).await? {
            if existing != receipt {
                return Err(DomainError::Conflict {
                    what: "execution_receipt",
                });
            }
            self.artifacts.protect_execution_receipt(&existing).await?;
            return Ok(ReconcileExecutionOperationOutcome::AlreadyReconciled(
                Box::new(existing),
            ));
        }

        self.artifacts.protect_execution_receipt(&receipt).await?;
        match self.store.record_receipt(receipt.clone()).await? {
            RecordExecutionReceiptOutcome::Recorded => Ok(
                ReconcileExecutionOperationOutcome::Reconciled(Box::new(receipt)),
            ),
            RecordExecutionReceiptOutcome::AlreadyRecorded => {
                let existing = self.store.receipt(receipt.operation_id()).await?.ok_or(
                    DomainError::InvariantViolated {
                        reason: "recorded execution receipt is unavailable",
                    },
                )?;
                if existing != receipt {
                    return Err(DomainError::Conflict {
                        what: "execution_receipt",
                    });
                }
                Ok(ReconcileExecutionOperationOutcome::AlreadyReconciled(
                    Box::new(existing),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use made_core::ports::{
        ArtifactChunkPage, ArtifactIdempotencyKey, ArtifactPage, ArtifactPageLimit, ArtifactRecord,
        ArtifactSnapshot, ArtifactStoreError, ArtifactStorePort, ArtifactTombstone,
        ArtifactUploadId, ArtifactUploadStatus, BeginArtifactUpload, ExecutionRecoveryPage,
        PutArtifactChunk, ReadArtifactChunk, RecordExecutionIntentOutcome, TombstoneArtifact,
    };
    use made_core::value_objects::{
        ArtifactDigest, ArtifactId, ArtifactMediaType, ArtifactProvenance, ArtifactRef,
        ArtifactSizeBytes, ArtifactSourceKind, AuditActorKind, CeremonyId, ExecutionConnectorId,
        ExecutionIntent, ExecutionOperation, ExecutionOperationId, ExecutionReceiptId,
        ExecutionReconciliationRequirement, ExecutionRecoveryCapability, ExecutionRecoveryCursor,
        ExecutionRecoveryPageLimit, ExecutionRequestBytes, StateIteration, StateVisit,
        StepClaimFence, StepId, StepIteration, StepOutput, StepResult,
    };
    use tokio::sync::Mutex;

    use super::*;

    struct ReceiptStore {
        intent: ExecutionIntent,
        requirement: Mutex<Option<ExecutionReconciliationRequirement>>,
        receipt: Mutex<Option<ExecutionReceipt>>,
    }

    #[async_trait]
    impl ExecutionReceiptStorePort for ReceiptStore {
        async fn record_intent(
            &self,
            _intent: ExecutionIntent,
        ) -> Result<RecordExecutionIntentOutcome, DomainError> {
            unreachable!()
        }
        async fn intent(
            &self,
            operation: &ExecutionOperationId,
            fence: &StepClaimFence,
        ) -> Result<Option<ExecutionIntent>, DomainError> {
            Ok((self.intent.operation().operation_id() == operation
                && self.intent.claim_fence() == fence)
                .then(|| self.intent.clone()))
        }
        async fn intents(
            &self,
            _operation: &ExecutionOperationId,
        ) -> Result<Vec<ExecutionIntent>, DomainError> {
            unreachable!()
        }
        async fn operation(
            &self,
            _operation: &ExecutionOperationId,
        ) -> Result<Option<ExecutionOperation>, DomainError> {
            unreachable!()
        }
        async fn receipt(
            &self,
            operation: &ExecutionOperationId,
        ) -> Result<Option<ExecutionReceipt>, DomainError> {
            Ok(self
                .receipt
                .lock()
                .await
                .as_ref()
                .filter(|receipt| receipt.operation_id() == operation)
                .cloned())
        }
        async fn record_reconciliation_requirement(
            &self,
            requirement: ExecutionReconciliationRequirement,
        ) -> Result<(), DomainError> {
            if !requirement.matches_intent(&self.intent) {
                return Err(DomainError::Conflict {
                    what: "execution_reconciliation_requirement",
                });
            }
            let mut stored = self.requirement.lock().await;
            match stored.as_ref() {
                Some(existing) if existing == &requirement => Ok(()),
                Some(_) => Err(DomainError::Conflict {
                    what: "execution_reconciliation_requirement",
                }),
                None => {
                    *stored = Some(requirement);
                    Ok(())
                }
            }
        }
        async fn reconciliation_requirement(
            &self,
            operation: &ExecutionOperationId,
            fence: &StepClaimFence,
        ) -> Result<Option<ExecutionReconciliationRequirement>, DomainError> {
            Ok(self
                .requirement
                .lock()
                .await
                .as_ref()
                .filter(|requirement| {
                    requirement.operation_id() == operation
                        && requirement.producer_claim_fence() == fence
                })
                .cloned())
        }
        async fn record_receipt(
            &self,
            receipt: ExecutionReceipt,
        ) -> Result<RecordExecutionReceiptOutcome, DomainError> {
            let mut stored = self.receipt.lock().await;
            match stored.as_ref() {
                Some(existing) if existing == &receipt => {
                    Ok(RecordExecutionReceiptOutcome::AlreadyRecorded)
                }
                Some(_) => Err(DomainError::Conflict {
                    what: "execution_receipt",
                }),
                None => {
                    *stored = Some(receipt);
                    Ok(RecordExecutionReceiptOutcome::Recorded)
                }
            }
        }
        async fn recoverable(
            &self,
            _after: Option<&ExecutionRecoveryCursor>,
            _limit: ExecutionRecoveryPageLimit,
        ) -> Result<ExecutionRecoveryPage, DomainError> {
            unreachable!()
        }
    }

    struct EvidenceStore(ArtifactRef);

    #[async_trait]
    impl ArtifactStorePort for EvidenceStore {
        async fn protect_references(
            &self,
            key: ArtifactIdempotencyKey,
            ids: Vec<ArtifactId>,
        ) -> Result<ArtifactSnapshot, ArtifactStoreError> {
            assert_eq!(ids, vec![self.0.artifact_id().clone()]);
            Ok(ArtifactSnapshot::protected(
                key,
                vec![ArtifactRecord {
                    artifact: self.0.clone(),
                    tombstone: None,
                    authorization: None,
                }],
            ))
        }
        async fn begin_upload(
            &self,
            _request: BeginArtifactUpload,
        ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
            unreachable!()
        }
        async fn put_chunk(
            &self,
            _request: PutArtifactChunk,
        ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
            unreachable!()
        }
        async fn artifact_id_for_upload(
            &self,
            _upload_id: &ArtifactUploadId,
        ) -> Result<ArtifactId, ArtifactStoreError> {
            unreachable!()
        }
        async fn commit_upload(
            &self,
            _upload_id: &ArtifactUploadId,
        ) -> Result<ArtifactRef, ArtifactStoreError> {
            unreachable!()
        }
        async fn abort_upload(
            &self,
            _upload_id: &ArtifactUploadId,
        ) -> Result<(), ArtifactStoreError> {
            unreachable!()
        }
        async fn get(
            &self,
            _artifact_id: &ArtifactId,
        ) -> Result<ArtifactRecord, ArtifactStoreError> {
            unreachable!()
        }
        async fn list(
            &self,
            _after: Option<&ArtifactId>,
            _limit: ArtifactPageLimit,
        ) -> Result<ArtifactPage, ArtifactStoreError> {
            unreachable!()
        }
        async fn read_chunk(
            &self,
            _request: ReadArtifactChunk,
        ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
            unreachable!()
        }
        async fn read_chunk_for_backup(
            &self,
            _request: ReadArtifactChunk,
        ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
            unreachable!()
        }
        async fn backup_content_available(
            &self,
            artifact_id: &ArtifactId,
        ) -> Result<bool, ArtifactStoreError> {
            Ok(artifact_id == self.0.artifact_id())
        }
        async fn tombstone(
            &self,
            _command: TombstoneArtifact,
        ) -> Result<ArtifactTombstone, ArtifactStoreError> {
            unreachable!()
        }
    }

    fn fixture_with(
        capability: ExecutionRecoveryCapability,
        ambiguous: bool,
    ) -> (
        Arc<ReceiptStore>,
        Arc<ArtifactService>,
        ReconcileExecutionOperationInput,
    ) {
        let operation = ExecutionOperation::new(
            CeremonyId::new("ceremony").unwrap(),
            StepId::new("effect").unwrap(),
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
            ExecutionRequestBytes::new(b"semantic request".to_vec()).unwrap(),
        );
        let fence = StepClaimFence::new("1".repeat(64)).unwrap();
        let connector = ExecutionConnectorId::new("manual-effect").unwrap();
        let intent = ExecutionIntent::new(
            operation.clone(),
            fence.clone(),
            connector.clone(),
            capability,
            ArtifactSourceKind::ExternalExecution,
            AuditActorKind::Engine,
            time::OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        let evidence = ArtifactRef::new(
            ArtifactId::new("operator-evidence").unwrap(),
            ArtifactDigest::new(format!("sha256:{}", "2".repeat(64))).unwrap(),
            ArtifactSizeBytes::new(1),
            ArtifactMediaType::new("application/json").unwrap(),
            ArtifactProvenance::execution(
                ArtifactSourceKind::ExternalExecution,
                ExecutionReceiptId::for_operation(operation.operation_id()),
                operation.operation_id().clone(),
                fence.clone(),
                time::OffsetDateTime::UNIX_EPOCH,
            )
            .unwrap(),
        );
        let requirement =
            ambiguous.then(|| ExecutionReconciliationRequirement::from_intent(&intent));
        let store = Arc::new(ReceiptStore {
            intent,
            requirement: Mutex::new(requirement),
            receipt: Mutex::new(None),
        });
        let artifacts = Arc::new(ArtifactService::new(Arc::new(EvidenceStore(
            evidence.clone(),
        ))));
        let input = ReconcileExecutionOperationInput {
            operation_id: operation.operation_id().clone(),
            request_digest: operation.request_digest().clone(),
            producer_claim_fence: fence,
            connector_id: connector,
            external_operation_id: None,
            result: StepResult::completed(StepOutput::empty()).unwrap(),
            evidence: vec![evidence],
            observed_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        (store, artifacts, input)
    }

    fn fixture() -> (
        Arc<ReceiptStore>,
        Arc<ArtifactService>,
        ReconcileExecutionOperationInput,
    ) {
        fixture_with(ExecutionRecoveryCapability::ReconciliationRequired, true)
    }

    #[tokio::test]
    async fn records_artifact_backed_reconciliation_once() {
        let (store, artifacts, input) = fixture();
        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts);
        assert!(matches!(
            use_case.execute(input.clone()).await.unwrap(),
            ReconcileExecutionOperationOutcome::Reconciled(_)
        ));
        assert!(matches!(
            use_case.execute(input).await.unwrap(),
            ReconcileExecutionOperationOutcome::AlreadyReconciled(_)
        ));
    }

    #[tokio::test]
    async fn rejects_wrong_digest_and_unbacked_declaration() {
        let (store, artifacts, mut input) = fixture();
        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts);
        input.request_digest =
            made_core::value_objects::ExecutionRequestDigest::new("3".repeat(64)).unwrap();
        assert!(matches!(
            use_case.execute(input.clone()).await,
            Err(DomainError::Conflict { .. })
        ));
        input.evidence.clear();
        assert!(matches!(
            use_case.execute(input).await,
            Err(DomainError::EmptyField { .. })
        ));
    }

    #[tokio::test]
    async fn queryable_intent_requires_a_real_ambiguity_marker() {
        let (store, artifacts, input) =
            fixture_with(ExecutionRecoveryCapability::QueryableByOperationId, false);
        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts);
        assert!(matches!(
            use_case.execute(input).await,
            Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement"
            })
        ));
    }

    #[tokio::test]
    async fn queryable_intent_with_connector_reported_ambiguity_is_reconciled_once() {
        let (store, artifacts, input) =
            fixture_with(ExecutionRecoveryCapability::QueryableByOperationId, true);
        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts);
        assert!(matches!(
            use_case.execute(input.clone()).await.unwrap(),
            ReconcileExecutionOperationOutcome::Reconciled(_)
        ));
        assert!(matches!(
            use_case.execute(input).await.unwrap(),
            ReconcileExecutionOperationOutcome::AlreadyReconciled(_)
        ));
    }

    #[tokio::test]
    async fn mismatched_ambiguity_marker_is_rejected() {
        let (store, artifacts, input) =
            fixture_with(ExecutionRecoveryCapability::QueryableByOperationId, true);
        let stored = store.requirement.lock().await.take().unwrap();
        let mut encoded = serde_json::to_value(stored).unwrap();
        encoded["request_digest"] = serde_json::Value::String("9".repeat(64));
        *store.requirement.lock().await = Some(serde_json::from_value(encoded).unwrap());

        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts);
        assert!(matches!(
            use_case.execute(input).await,
            Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement"
            })
        ));
    }

    #[tokio::test]
    async fn concurrent_identical_reconciliation_records_one_receipt() {
        let (store, artifacts, input) =
            fixture_with(ExecutionRecoveryCapability::QueryableByOperationId, true);
        let use_case = Arc::new(ReconcileExecutionOperationUseCase::new(store, artifacts));
        let (left, right) = tokio::join!(use_case.execute(input.clone()), use_case.execute(input),);
        let outcomes = [left.unwrap(), right.unwrap()];
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    ReconcileExecutionOperationOutcome::Reconciled(_)
                ))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    ReconcileExecutionOperationOutcome::AlreadyReconciled(_)
                ))
                .count(),
            1
        );
    }
}
