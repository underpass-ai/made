use std::sync::Arc;

use anyhow::{bail, Context, Result};
use made_adapters::artifacts::LocalArtifactStore;
use made_adapters::config::ServiceConfig;
use made_adapters::postgres::{
    PostgresArtifactStore, PostgresCeremonyStore, PostgresConfig, PostgresPool,
};
use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::artifacts::ArtifactService;
use made_app::services::AuthorizationOperationScope;
use made_app::workers::{
    ReconcileExecutionOperationInput, ReconcileExecutionOperationOutcome,
    ReconcileExecutionOperationUseCase,
};
use made_core::ports::{ArtifactIdempotencyKey, ArtifactStorePort, ExecutionReceiptStorePort};
use made_core::value_objects::{ArtifactMediaType, AuthorizationAction, ExecutionReceipt};
use serde_json::{json, Value};
use time::OffsetDateTime;

pub(super) async fn execute(config: &ServiceConfig, receipt: &ExecutionReceipt) -> Result<Value> {
    let operation =
        AuthorizationOperationScope::current().context("reconciliation requires authorization")?;
    if operation.evidence().action() != AuthorizationAction::ReconcileExecutionOperation {
        bail!("reconciliation requires its dedicated action");
    }
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
    let intent = receipts
        .intent(receipt.operation_id(), receipt.producer_claim_fence())
        .await?
        .context("reconciliation requires the exact persisted intent")?;
    if receipt.source_kind() != intent.source_kind()
        || receipt.recovery_capability() != intent.recovery_capability()
        || !receipt.budget_measurement().is_unknown()
    {
        bail!("declared receipt must match the intent and must not assert unsupported usage measurements");
    }
    let artifacts = Arc::new(ArtifactService::new(artifact_store.clone()));
    // Keep actor, authorization and the exact declaration durable before the
    // receipt write. This is an attempt record, never a claim of success.
    let audit_key =
        ArtifactIdempotencyKey::new(format!("reconciliation-audit:{}", uuid::Uuid::new_v4()))?;
    let audit = artifacts.save_generated_report(
        &serde_json::to_vec(&json!({"phase":"authorized_attempt", "authorization":operation.evidence(), "receipt":receipt}))?,
        ArtifactMediaType::new("application/json")?,
        OffsetDateTime::now_utc(),
        audit_key.clone(),
    ).await?;
    artifact_store
        .protect_references(audit_key, vec![audit.artifact_id().clone()])
        .await?;
    let input = ReconcileExecutionOperationInput {
        operation_id: receipt.operation_id().clone(),
        request_digest: receipt.request_digest().clone(),
        producer_claim_fence: receipt.producer_claim_fence().clone(),
        connector_id: receipt.connector_id().clone(),
        external_operation_id: receipt.external_operation_id().cloned(),
        result: receipt.result().clone(),
        evidence: receipt.artifacts().to_vec(),
        observed_at: receipt.observed_at(),
    };
    let result = ReconcileExecutionOperationUseCase::new(receipts, artifacts)
        .execute(input)
        .await?;
    let (receipt, already_recorded) = match result {
        ReconcileExecutionOperationOutcome::Reconciled(receipt) => (receipt, false),
        ReconcileExecutionOperationOutcome::AlreadyReconciled(receipt) => (receipt, true),
    };
    Ok(json!({"receipt":receipt,"already_recorded":already_recorded,"authorization_audit":audit}))
}
