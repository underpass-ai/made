use std::sync::Arc;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use made_adapters::artifacts::LocalArtifactStore;
use made_adapters::config::ServiceConfig;
use made_adapters::postgres::{
    PostgresArtifactStore, PostgresCeremonyStore, PostgresConfig, PostgresPool,
};
use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::artifacts::ArtifactService;
use made_app::services::AuthorizationOperationScope;
use made_app::workers::{
    ExecutionReconciliationAuditPort, ExecutionReconciliationAuditRecord,
    ReconcileExecutionOperationInput, ReconcileExecutionOperationOutcome,
    ReconcileExecutionOperationUseCase,
};
use made_core::ports::{ArtifactIdempotencyKey, ArtifactStorePort, ExecutionReceiptStorePort};
use made_core::value_objects::{ArtifactMediaType, AuthorizedOperation, ExecutionReceipt};
use made_core::DomainError;
use serde_json::{json, Value};
use time::OffsetDateTime;

pub(super) async fn execute(config: &ServiceConfig, receipt: &ExecutionReceipt) -> Result<Value> {
    let operation =
        AuthorizationOperationScope::current().context("reconciliation requires authorization")?;
    receipt.validate()?;
    let (receipts, artifact_store): (
        Arc<dyn ExecutionReceiptStorePort>,
        Arc<dyn ArtifactStorePort>,
    ) = if let Some(url) = &config.postgres_url {
        let pool = PostgresPool::connect(&PostgresConfig::from_url(url.clone())).await?;
        (
            Arc::new(PostgresCeremonyStore::new(pool.clone())),
            Arc::new(PostgresArtifactStore::new(pool)),
        )
    } else {
        let database = config
            .ceremony_store_path
            .as_ref()
            .context("MADE_CEREMONY_STORE_PATH is required")?;
        let artifacts = config
            .artifact_store_path
            .as_ref()
            .context("MADE_ARTIFACT_STORE_PATH is required")?;
        if !std::path::Path::new(database).is_file() || !std::path::Path::new(artifacts).is_dir() {
            bail!("reconciliation stores must already exist");
        }
        (
            Arc::new(SqliteCeremonyStore::open(database)?),
            Arc::new(LocalArtifactStore::open(artifacts)?),
        )
    };
    let artifacts = Arc::new(ArtifactService::new(artifact_store.clone()));
    let audit = Arc::new(MaintenanceReconciliationAudit {
        artifacts: artifacts.clone(),
        store: artifact_store,
    });
    let input = ReconcileExecutionOperationInput {
        authorization: operation,
        receipt: receipt.clone(),
    };
    let result = ReconcileExecutionOperationUseCase::new(receipts, artifacts, audit)
        .execute(input)
        .await?;
    let (receipt, already_recorded, authorization_audit) = match result {
        ReconcileExecutionOperationOutcome::Reconciled {
            receipt,
            authorization_audit,
        } => (receipt, false, authorization_audit),
        ReconcileExecutionOperationOutcome::AlreadyReconciled {
            receipt,
            authorization_audit,
        } => (receipt, true, authorization_audit),
    };
    Ok(
        json!({"receipt":receipt,"already_recorded":already_recorded,"authorization_audit":authorization_audit}),
    )
}

struct MaintenanceReconciliationAudit {
    artifacts: Arc<ArtifactService>,
    store: Arc<dyn ArtifactStorePort>,
}

#[async_trait]
impl ExecutionReconciliationAuditPort for MaintenanceReconciliationAudit {
    async fn record_authorized_attempt(
        &self,
        authorization: &AuthorizedOperation,
        receipt: &ExecutionReceipt,
    ) -> Result<ExecutionReconciliationAuditRecord, DomainError> {
        let key =
            ArtifactIdempotencyKey::new(format!("reconciliation-audit:{}", uuid::Uuid::new_v4()))?;
        let bytes = serde_json::to_vec(&json!({
            "phase": "authorized_attempt",
            "authorization": authorization.evidence(),
            "receipt": receipt,
        }))
        .map_err(|_| DomainError::InvariantViolated {
            reason: "reconciliation audit could not be encoded",
        })?;
        let artifact = self
            .artifacts
            .save_generated_report(
                &bytes,
                ArtifactMediaType::new("application/json")?,
                OffsetDateTime::now_utc(),
                key.clone(),
            )
            .await
            .map_err(audit_storage_error)?;
        Ok(ExecutionReconciliationAuditRecord::new(artifact, key))
    }

    async fn protect_authorized_attempt(
        &self,
        audit: &ExecutionReconciliationAuditRecord,
    ) -> Result<(), DomainError> {
        self.store
            .protect_references(
                audit.protection_key().clone(),
                vec![audit.artifact().artifact_id().clone()],
            )
            .await
            .map(|_| ())
            .map_err(audit_storage_error)
    }
}

fn audit_storage_error<T>(_error: T) -> DomainError {
    DomainError::InvariantViolated {
        reason: "reconciliation authorization audit storage failed",
    }
}
