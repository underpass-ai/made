use std::sync::Arc;

use anyhow::{bail, Context, Result};
use made_adapters::artifacts::{LocalArtifactStore, SqliteArtifactBackupService};
use made_adapters::config::ServiceConfig;
use made_core::ports::ArtifactStorePort;
use made_core::value_objects::{DurationMs, IdempotencyKey, LeaseOwnerId, StepLease};
use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::maintenance_command::MaintenanceCommand;

pub(super) async fn execute(config: &ServiceConfig, command: &MaintenanceCommand) -> Result<Value> {
    let artifact_path = config
        .artifact_store_path
        .as_ref()
        .context("MADE_ARTIFACT_STORE_PATH is required")?;
    if !std::path::Path::new(artifact_path).is_dir() {
        bail!("the configured artifact store must already exist");
    }
    let store = Arc::new(LocalArtifactStore::open(artifact_path)?);
    match command {
        MaintenanceCommand::Backup { destination, key } => {
            let database = config
                .ceremony_store_path
                .as_ref()
                .context("MADE_CEREMONY_STORE_PATH is required")?;
            let service = SqliteArtifactBackupService::new((*store).clone(), database);
            Ok(serde_json::to_value(
                service.backup_to(destination, key.clone()).await?,
            )?)
        }
        MaintenanceCommand::Restore {
            source,
            destination,
        } => {
            let database = config
                .ceremony_store_path
                .as_ref()
                .context("MADE_CEREMONY_STORE_PATH is required")?;
            SqliteArtifactBackupService::new((*store).clone(), database)
                .restore_set_to(source, destination)
                .await?;
            Ok(json!({"restored_to":destination,"verified":true}))
        }
        MaintenanceCommand::RestorePostgres { .. } => {
            bail!("PostgreSQL restore requires PostgreSQL source configuration")
        }
        MaintenanceCommand::ReconcileExecution { .. } => {
            bail!("reconciliation uses its dedicated admission path")
        }
        MaintenanceCommand::GcPreview { retired_before } => {
            let cutoff = OffsetDateTime::parse(retired_before, &Rfc3339)?;
            let now = OffsetDateTime::now_utc();
            let lease = StepLease::acquire(
                LeaseOwnerId::new("local-maintenance")?,
                IdempotencyKey::new(format!("gc:{}", uuid::Uuid::new_v4()))?,
                now,
                DurationMs::from_millis(300_000),
            )?;
            let plan = store.plan_gc(cutoff, lease).await?;
            Ok(json!({"plan":plan,"preview":LocalArtifactStore::dry_run_gc(&plan)}))
        }
        MaintenanceCommand::GcApply { plan } => Ok(serde_json::to_value(
            store.apply_gc(plan, OffsetDateTime::now_utc()).await?,
        )?),
        MaintenanceCommand::ReleaseProtection { key, reason } => {
            if reason.trim().is_empty() {
                bail!("release requires an explicit abandonment reason");
            }
            if key.as_str().starts_with("receipt:")
                || key.as_str().starts_with("restore:")
                || key.as_str().starts_with("reconciliation-audit:")
            {
                bail!("receipt and restored-store protections require an explicit reference-retirement workflow");
            }
            store.release_snapshot(key).await?;
            Ok(json!({"released":key,"reason":reason}))
        }
    }
}
