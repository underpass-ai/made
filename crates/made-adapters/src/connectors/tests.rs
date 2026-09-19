use super::*;
use made_core::value_objects::{ExecutionConnectorId, ExecutionRecoveryCapability};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_core::value_objects::{
    ArtifactSourceKind, AuditActorKind, CeremonyId, ExecutionIntent, ExecutionOperation,
    ExecutionRequestBytes, ExternalOperationId, StateIteration, StateVisit, StepClaimFence, StepId,
    StepIteration,
};
use time::OffsetDateTime;

#[test]
fn descriptors_reject_missing_capabilities() {
    let error = ConnectorDescriptor::new(
        ExecutionConnectorId::new("filesystem-test").unwrap(),
        ConnectorKind::Filesystem,
        ConnectorCapabilities::empty(),
        None,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ConnectorContractError::MissingCapabilities { .. }
    ));
}

#[test]
fn repository_connector_must_advertise_recovery_when_queryable() {
    let error = ConnectorDescriptor::new(
        ExecutionConnectorId::new("script-test").unwrap(),
        ConnectorKind::RepositoryScript,
        ConnectorCapabilities::new([ConnectorCapability::Execute]),
        Some(ExecutionRecoveryCapability::QueryableByOperationId),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ConnectorContractError::MissingCapabilities { .. }
    ));
}

fn intent(request: &[u8]) -> ExecutionIntent {
    let operation = ExecutionOperation::new(
        CeremonyId::new("connectors-test").unwrap(),
        StepId::new("script").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
        ExecutionRequestBytes::new(request.to_vec()).unwrap(),
    );
    ExecutionIntent::new(
        operation,
        StepClaimFence::new("1".repeat(64)).unwrap(),
        ExecutionConnectorId::new("repo.script.v1").unwrap(),
        ExecutionRecoveryCapability::QueryableByOperationId,
        ArtifactSourceKind::ExternalExecution,
        AuditActorKind::Engine,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}

fn descriptor(
    id: &str,
    kind: ConnectorKind,
    capabilities: impl IntoIterator<Item = ConnectorCapability>,
    recovery: Option<ExecutionRecoveryCapability>,
) -> ConnectorDescriptor {
    ConnectorDescriptor::new(
        ExecutionConnectorId::new(id).unwrap(),
        kind,
        ConnectorCapabilities::new(capabilities),
        recovery,
    )
    .unwrap()
}

#[derive(Debug)]
struct ScriptDouble {
    descriptor: ConnectorDescriptor,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl RepositoryScriptConnector for ScriptDouble {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    async fn execute_or_recover(
        &self,
        invocation: ScriptInvocation,
    ) -> Result<ScriptResolution, ConnectorError> {
        assert_eq!(invocation.request_bytes(), b"secret=never-in-receipt");
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ScriptResolution::Observed(ScriptObservation::new(
            invocation.claim_fence().clone(),
            Some(ExternalOperationId::new("external-op").unwrap()),
            ReceiptStatus::Completed,
            12,
            4,
            OffsetDateTime::UNIX_EPOCH,
        )))
    }
}

#[derive(Debug)]
struct ReceiptDouble {
    descriptor: ConnectorDescriptor,
    stored: Arc<Mutex<Option<SafeExecutionReceipt>>>,
}

#[async_trait]
impl ReceiptFilesystemConnector for ReceiptDouble {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    async fn load(
        &self,
        operation_id: &made_core::value_objects::ExecutionOperationId,
    ) -> Result<Option<SafeExecutionReceipt>, ConnectorError> {
        Ok(self
            .stored
            .lock()
            .unwrap()
            .as_ref()
            .filter(|receipt| receipt.operation_id() == operation_id)
            .cloned())
    }

    async fn store(&self, receipt: &SafeExecutionReceipt) -> Result<(), ConnectorError> {
        *self.stored.lock().unwrap() = Some(receipt.clone());
        Ok(())
    }
}

#[derive(Debug)]
struct TransportDouble {
    descriptor: ConnectorDescriptor,
    published: Arc<Mutex<Vec<SafeExecutionReceipt>>>,
}

#[async_trait]
impl ReceiptTransportConnector for TransportDouble {
    fn descriptor(&self) -> &ConnectorDescriptor {
        &self.descriptor
    }

    async fn publish(&self, receipt: &SafeExecutionReceipt) -> Result<(), ConnectorError> {
        self.published.lock().unwrap().push(receipt.clone());
        Ok(())
    }
}

#[tokio::test]
async fn durable_connector_reuses_receipt_without_reexecuting_secret_input() {
    let calls = Arc::new(AtomicUsize::new(0));
    let stored = Arc::new(Mutex::new(None));
    let published = Arc::new(Mutex::new(Vec::new()));
    let connector = DurableExecutionConnector::new(
        ScriptDouble {
            descriptor: descriptor(
                "repo.script.v1",
                ConnectorKind::RepositoryScript,
                [ConnectorCapability::Execute, ConnectorCapability::Recover],
                Some(ExecutionRecoveryCapability::QueryableByOperationId),
            ),
            calls: calls.clone(),
        },
        ReceiptDouble {
            descriptor: descriptor(
                "receipts.filesystem.v1",
                ConnectorKind::Filesystem,
                [
                    ConnectorCapability::ReadReceipt,
                    ConnectorCapability::WriteReceipt,
                    ConnectorCapability::AtomicReceipt,
                ],
                None,
            ),
            stored: stored.clone(),
        },
        TransportDouble {
            descriptor: descriptor(
                "transport.test.v1",
                ConnectorKind::Transport,
                [
                    ConnectorCapability::PublishReceipt,
                    ConnectorCapability::ConfirmDelivery,
                ],
                None,
            ),
            published: published.clone(),
        },
    )
    .unwrap();
    let request = intent(b"secret=never-in-receipt");

    let first = connector.execute_or_recover(&request).await.unwrap();
    let DurableExecutionResult::Receipt(receipt) = first else {
        panic!("expected a terminal receipt");
    };
    let json = serde_json::to_string(&receipt).unwrap();
    assert!(!json.contains("secret=never-in-receipt"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let second = connector.execute_or_recover(&request).await.unwrap();
    assert!(matches!(second, DurableExecutionResult::Receipt(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(published.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn json_receipt_store_round_trips_without_request_bytes() {
    let directory = tempfile::TempDir::new().unwrap();
    let store = JsonFileReceiptStore::new(
        ExecutionConnectorId::new("receipts.filesystem.v1").unwrap(),
        directory.path(),
    )
    .unwrap();
    let request = intent(b"secret=never-in-receipt");
    let receipt = SafeExecutionReceipt::from_observation(
        &request,
        ExecutionConnectorId::new("repo.script.v1").unwrap(),
        ExecutionRecoveryCapability::QueryableByOperationId,
        ScriptObservation::new(
            request.claim_fence().clone(),
            None,
            ReceiptStatus::Failed,
            0,
            0,
            OffsetDateTime::UNIX_EPOCH,
        ),
    )
    .unwrap();
    store.store(&receipt).await.unwrap();
    assert_eq!(
        store
            .load(request.operation().operation_id())
            .await
            .unwrap(),
        Some(receipt)
    );
    let persisted = std::fs::read_dir(directory.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert!(!std::fs::read_to_string(persisted.path())
        .unwrap()
        .contains("secret=never-in-receipt"));
}
