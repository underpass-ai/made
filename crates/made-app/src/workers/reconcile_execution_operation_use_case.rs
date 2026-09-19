use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{ExecutionReceiptStorePort, RecordExecutionReceiptOutcome};
use made_core::value_objects::AuthorizationAction;

use super::{
    ExecutionReconciliationAuditPort, ReconcileExecutionOperationInput,
    ReconcileExecutionOperationOutcome,
};
use crate::artifacts::ArtifactService;

/// Resolve one ambiguous effect from explicit, artifact-backed operator evidence.
/// This path never invokes an execution connector or removes the original intent.
pub struct ReconcileExecutionOperationUseCase {
    store: Arc<dyn ExecutionReceiptStorePort>,
    artifacts: Arc<ArtifactService>,
    audit: Arc<dyn ExecutionReconciliationAuditPort>,
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
    pub fn new(
        store: Arc<dyn ExecutionReceiptStorePort>,
        artifacts: Arc<ArtifactService>,
        audit: Arc<dyn ExecutionReconciliationAuditPort>,
    ) -> Self {
        Self {
            store,
            artifacts,
            audit,
        }
    }

    pub async fn execute(
        &self,
        input: ReconcileExecutionOperationInput,
    ) -> Result<ReconcileExecutionOperationOutcome, DomainError> {
        if input.authorization.evidence().action()
            != AuthorizationAction::ReconcileExecutionOperation
        {
            return Err(DomainError::InvariantViolated {
                reason: "execution reconciliation requires its dedicated authorization action",
            });
        }
        input.receipt.validate()?;
        if input.receipt.artifacts().is_empty() {
            return Err(DomainError::EmptyField {
                field: "execution_reconciliation.evidence",
            });
        }
        let receipt = input.receipt;
        let intent = self
            .store
            .intent(receipt.operation_id(), receipt.producer_claim_fence())
            .await?
            .ok_or(DomainError::NotFound {
                what: "execution_intent",
            })?;
        if intent.operation().operation_id() != receipt.operation_id()
            || intent.operation().request_digest() != receipt.request_digest()
            || intent.claim_fence() != receipt.producer_claim_fence()
            || intent.connector_id() != receipt.connector_id()
            || intent.source_kind() != receipt.source_kind()
            || intent.recovery_capability() != receipt.recovery_capability()
            || !receipt.budget_measurement().is_unknown()
        {
            return Err(DomainError::Conflict {
                what: "execution_reconciliation",
            });
        }
        let requirement = self
            .store
            .reconciliation_requirement(receipt.operation_id(), receipt.producer_claim_fence())
            .await?
            .ok_or(DomainError::Conflict {
                what: "execution_reconciliation_requirement",
            })?;
        if !requirement.matches_intent(&intent)
            || requirement.operation_id() != receipt.operation_id()
            || requirement.request_digest() != receipt.request_digest()
            || requirement.producer_claim_fence() != receipt.producer_claim_fence()
            || requirement.connector_id() != receipt.connector_id()
        {
            return Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement",
            });
        }

        let audit = self
            .audit
            .record_authorized_attempt(&input.authorization, &receipt)
            .await?;
        self.audit.protect_authorized_attempt(&audit).await?;
        if let Some(existing) = self.store.receipt(receipt.operation_id()).await? {
            if existing != receipt {
                return Err(DomainError::Conflict {
                    what: "execution_receipt",
                });
            }
            self.artifacts.protect_execution_receipt(&existing).await?;
            return Ok(ReconcileExecutionOperationOutcome::AlreadyReconciled {
                receipt: Box::new(existing),
                authorization_audit: audit.artifact().clone(),
            });
        }

        self.artifacts.protect_execution_receipt(&receipt).await?;
        match self.store.record_receipt(receipt.clone()).await? {
            RecordExecutionReceiptOutcome::Recorded => {
                Ok(ReconcileExecutionOperationOutcome::Reconciled {
                    receipt: Box::new(receipt),
                    authorization_audit: audit.artifact().clone(),
                })
            }
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
                Ok(ReconcileExecutionOperationOutcome::AlreadyReconciled {
                    receipt: Box::new(existing),
                    authorization_audit: audit.artifact().clone(),
                })
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
        ArtifactSizeBytes, ArtifactSourceKind, AuditActorKind, AuthenticatedPrincipal,
        AuthenticationMethod, AuthorizationEvidence, AuthorizedOperation, CeremonyId,
        ExecutionConnectorId, ExecutionIntent, ExecutionOperation, ExecutionOperationId,
        ExecutionReceipt, ExecutionReceiptId, ExecutionReconciliationRequirement,
        ExecutionRecoveryCapability, ExecutionRecoveryCursor, ExecutionRecoveryPageLimit,
        ExecutionRequestBytes, PrincipalId, PrincipalKind, StateIteration, StateVisit,
        StepClaimFence, StepId, StepIteration, StepOutput, StepResult,
    };
    use tokio::sync::Mutex;

    use super::*;
    use crate::workers::ExecutionReconciliationAuditRecord;

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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AuditFailure {
        Record,
        Protect,
    }

    struct AuditPort {
        artifact: ArtifactRef,
        failure: Option<AuditFailure>,
        calls: Mutex<Vec<&'static str>>,
    }

    #[async_trait]
    impl ExecutionReconciliationAuditPort for AuditPort {
        async fn record_authorized_attempt(
            &self,
            authorization: &AuthorizedOperation,
            receipt: &ExecutionReceipt,
        ) -> Result<ExecutionReconciliationAuditRecord, DomainError> {
            assert_eq!(
                authorization.evidence().action(),
                AuthorizationAction::ReconcileExecutionOperation
            );
            receipt.validate()?;
            self.calls.lock().await.push("record");
            if self.failure == Some(AuditFailure::Record) {
                return Err(DomainError::InvariantViolated {
                    reason: "injected audit record failure",
                });
            }
            Ok(ExecutionReconciliationAuditRecord::new(
                self.artifact.clone(),
                ArtifactIdempotencyKey::new("reconciliation-audit-test")?,
            ))
        }

        async fn protect_authorized_attempt(
            &self,
            audit: &ExecutionReconciliationAuditRecord,
        ) -> Result<(), DomainError> {
            assert_eq!(audit.artifact(), &self.artifact);
            self.calls.lock().await.push("protect");
            if self.failure == Some(AuditFailure::Protect) {
                return Err(DomainError::InvariantViolated {
                    reason: "injected audit protection failure",
                });
            }
            Ok(())
        }
    }

    fn authorization(action: &str) -> AuthorizedOperation {
        let principal = AuthenticatedPrincipal::new(
            PrincipalId::new("operator").unwrap(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::MutualTls,
        )
        .unwrap();
        let evidence: AuthorizationEvidence = serde_json::from_value(serde_json::json!({
            "decision_id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "request_id": "reconciliation-request",
            "principal_id": "operator",
            "action": action,
            "scope": { "kind": "global" },
            "target_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "policy_version": 1,
            "admitted_at": "2026-09-19T12:00:00Z",
            "valid_until": "2026-09-19T12:01:00Z"
        }))
        .unwrap();
        AuthorizedOperation::new(principal, evidence).unwrap()
    }

    fn fixture_with(
        capability: ExecutionRecoveryCapability,
        ambiguous: bool,
    ) -> (
        Arc<ReceiptStore>,
        Arc<ArtifactService>,
        Arc<AuditPort>,
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
        let receipt = ExecutionReceipt::new(
            operation.operation_id().clone(),
            operation.request_digest().clone(),
            fence,
            connector,
            None,
            capability,
            ArtifactSourceKind::ExternalExecution,
            StepResult::completed(StepOutput::empty()).unwrap(),
            vec![evidence.clone()],
            time::OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        let audit = Arc::new(AuditPort {
            artifact: evidence,
            failure: None,
            calls: Mutex::new(Vec::new()),
        });
        let input = ReconcileExecutionOperationInput {
            authorization: authorization("reconcile_execution_operation"),
            receipt,
        };
        (store, artifacts, audit, input)
    }

    fn fixture() -> (
        Arc<ReceiptStore>,
        Arc<ArtifactService>,
        Arc<AuditPort>,
        ReconcileExecutionOperationInput,
    ) {
        fixture_with(ExecutionRecoveryCapability::ReconciliationRequired, true)
    }

    #[tokio::test]
    async fn records_artifact_backed_reconciliation_once() {
        let (store, artifacts, audit, input) = fixture();
        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts, audit.clone());
        assert!(matches!(
            use_case.execute(input.clone()).await.unwrap(),
            ReconcileExecutionOperationOutcome::Reconciled { .. }
        ));
        assert!(matches!(
            use_case.execute(input).await.unwrap(),
            ReconcileExecutionOperationOutcome::AlreadyReconciled { .. }
        ));
        assert_eq!(
            audit.calls.lock().await.as_slice(),
            ["record", "protect", "record", "protect"]
        );
    }

    #[tokio::test]
    async fn rejects_wrong_digest_and_unbacked_declaration() {
        let (store, artifacts, audit, mut input) = fixture();
        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts, audit);
        let mut receipt = serde_json::to_value(&input.receipt).unwrap();
        receipt["request_digest"] = serde_json::Value::String("3".repeat(64));
        input.receipt = serde_json::from_value(receipt).unwrap();
        assert!(matches!(
            use_case.execute(input.clone()).await,
            Err(DomainError::Conflict { .. })
        ));
        let mut receipt = serde_json::to_value(&input.receipt).unwrap();
        receipt["artifacts"] = serde_json::Value::Array(Vec::new());
        input.receipt = serde_json::from_value(receipt).unwrap();
        assert!(matches!(
            use_case.execute(input).await,
            Err(DomainError::EmptyField { .. })
        ));
    }

    #[tokio::test]
    async fn rejects_wrong_authorization_before_writing_audit_or_receipt() {
        let (store, artifacts, audit, mut input) = fixture();
        input.authorization = authorization("backup_store");
        let use_case =
            ReconcileExecutionOperationUseCase::new(store.clone(), artifacts, audit.clone());

        assert!(use_case.execute(input).await.is_err());
        assert!(audit.calls.lock().await.is_empty());
        assert!(store.receipt.lock().await.is_none());
    }

    #[tokio::test]
    async fn audit_record_or_protection_failure_prevents_receipt_write() {
        for failure in [AuditFailure::Record, AuditFailure::Protect] {
            let (store, artifacts, template, input) = fixture();
            let audit = Arc::new(AuditPort {
                artifact: template.artifact.clone(),
                failure: Some(failure),
                calls: Mutex::new(Vec::new()),
            });
            let use_case =
                ReconcileExecutionOperationUseCase::new(store.clone(), artifacts, audit.clone());

            assert!(use_case.execute(input).await.is_err());
            let expected = if failure == AuditFailure::Record {
                vec!["record"]
            } else {
                vec!["record", "protect"]
            };
            assert_eq!(*audit.calls.lock().await, expected);
            assert!(store.receipt.lock().await.is_none());
        }
    }

    #[tokio::test]
    async fn queryable_intent_requires_a_real_ambiguity_marker() {
        let (store, artifacts, audit, input) =
            fixture_with(ExecutionRecoveryCapability::QueryableByOperationId, false);
        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts, audit);
        assert!(matches!(
            use_case.execute(input).await,
            Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement"
            })
        ));
    }

    #[tokio::test]
    async fn queryable_intent_with_connector_reported_ambiguity_is_reconciled_once() {
        let (store, artifacts, audit, input) =
            fixture_with(ExecutionRecoveryCapability::QueryableByOperationId, true);
        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts, audit);
        assert!(matches!(
            use_case.execute(input.clone()).await.unwrap(),
            ReconcileExecutionOperationOutcome::Reconciled { .. }
        ));
        assert!(matches!(
            use_case.execute(input).await.unwrap(),
            ReconcileExecutionOperationOutcome::AlreadyReconciled { .. }
        ));
    }

    #[tokio::test]
    async fn mismatched_ambiguity_marker_is_rejected() {
        let (store, artifacts, audit, input) =
            fixture_with(ExecutionRecoveryCapability::QueryableByOperationId, true);
        let stored = store.requirement.lock().await.take().unwrap();
        let mut encoded = serde_json::to_value(stored).unwrap();
        encoded["request_digest"] = serde_json::Value::String("9".repeat(64));
        *store.requirement.lock().await = Some(serde_json::from_value(encoded).unwrap());

        let use_case = ReconcileExecutionOperationUseCase::new(store, artifacts, audit);
        assert!(matches!(
            use_case.execute(input).await,
            Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement"
            })
        ));
    }

    #[tokio::test]
    async fn concurrent_identical_reconciliation_records_one_receipt() {
        let (store, artifacts, audit, input) =
            fixture_with(ExecutionRecoveryCapability::QueryableByOperationId, true);
        let use_case = Arc::new(ReconcileExecutionOperationUseCase::new(
            store, artifacts, audit,
        ));
        let (left, right) = tokio::join!(use_case.execute(input.clone()), use_case.execute(input),);
        let outcomes = [left.unwrap(), right.unwrap()];
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    ReconcileExecutionOperationOutcome::Reconciled { .. }
                ))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    ReconcileExecutionOperationOutcome::AlreadyReconciled { .. }
                ))
                .count(),
            1
        );
    }
}
