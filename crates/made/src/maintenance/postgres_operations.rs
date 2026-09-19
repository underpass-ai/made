use anyhow::{bail, Context, Result};
use made_adapters::artifacts::ArtifactGcReport;
use made_adapters::config::ServiceConfig;
use made_adapters::postgres::{
    PostgresArtifactStore, PostgresBackupService, PostgresConfig, PostgresPool,
};
use made_core::ports::ArtifactStorePort;
use made_core::value_objects::{DurationMs, IdempotencyKey, LeaseOwnerId, StepLease};
use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::maintenance_command::MaintenanceCommand;

pub(super) async fn execute(
    config: &ServiceConfig,
    command: &MaintenanceCommand,
    resolved_target_url: Option<&str>,
) -> Result<Value> {
    let source_url = config
        .postgres_url
        .as_ref()
        .context("MADE_POSTGRES_URL is required")?;
    let pool = PostgresPool::connect(&PostgresConfig::from_url(source_url.clone())).await?;
    let store = PostgresArtifactStore::new(pool);
    match command {
        MaintenanceCommand::Backup { destination, key } => {
            let manifest = PostgresBackupService::new(store, source_url.clone())
                .backup_to(destination, key.clone())
                .await?;
            Ok(serde_json::to_value(manifest)?)
        }
        MaintenanceCommand::RestorePostgres { source, .. } => {
            let target = resolved_target_url.context("restore target is not configured")?;
            PostgresBackupService::new(store, source_url.clone())
                .restore_to(source, target)
                .await?;
            Ok(json!({"restored_to_identity":"isolated_postgres","verified":true}))
        }
        MaintenanceCommand::Restore { .. } => {
            bail!("PostgreSQL restore requires restore_postgres and a trusted target URL environment variable")
        }
        MaintenanceCommand::GcPreview { retired_before } => {
            let cutoff = OffsetDateTime::parse(retired_before, &Rfc3339)?;
            let now = OffsetDateTime::now_utc();
            let lease = StepLease::acquire(
                LeaseOwnerId::new("postgres-maintenance")?,
                IdempotencyKey::new(format!("gc:{}", uuid::Uuid::new_v4()))?,
                now,
                DurationMs::from_millis(300_000),
            )?;
            let plan = store.plan_gc(cutoff, lease).await?;
            Ok(json!({"plan":plan,"preview":ArtifactGcReport::dry_run(&plan)}))
        }
        MaintenanceCommand::GcApply { plan } => Ok(serde_json::to_value(
            store.apply_gc(plan, OffsetDateTime::now_utc()).await?,
        )?),
        MaintenanceCommand::ReleaseProtection { key, reason } => {
            if reason.trim().is_empty() {
                bail!("release requires an explicit abandonment reason");
            }
            if key.as_str().starts_with("receipt:") || key.as_str().starts_with("restore:") {
                bail!("receipt and restored-store protections require an explicit reference-retirement workflow");
            }
            if key.as_str().starts_with("reconciliation-audit:") {
                bail!("reconciliation audit protections require an explicit audit-retirement workflow");
            }
            store.release_snapshot(key).await?;
            Ok(json!({"released":key,"reason":reason}))
        }
        MaintenanceCommand::ReconcileExecution { .. } => {
            bail!("execution reconciliation must be dispatched before storage maintenance")
        }
    }
}
