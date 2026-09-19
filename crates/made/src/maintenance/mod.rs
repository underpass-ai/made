//! Local administrative operations, admitted by the installed host policy.

mod authorization;
mod maintenance_command;
mod maintenance_request;
mod postgres_operations;
mod reconciliation;
mod sqlite_operations;

use anyhow::{bail, Context, Result};
use made_adapters::config::EnvConfiguration;
use made_app::services::AuthorizationOperationScope;
use made_core::value_objects::AuthorizationTargetDigest;
use serde_json::json;

use maintenance_request::MaintenanceRequest;

/// Execute a JSON request or describe its exact authorization target.
pub async fn run(arguments: Vec<String>) -> Result<()> {
    let [mode, path] = arguments.as_slice() else {
        bail!("usage: made maintenance --request|--describe REQUEST.json");
    };
    if mode != "--request" && mode != "--describe" {
        bail!("expected --request or --describe");
    }
    let bytes = std::fs::read(path).context("cannot read maintenance request")?;
    let request: MaintenanceRequest =
        serde_json::from_slice(&bytes).context("invalid maintenance request")?;
    let config = EnvConfiguration::new().load()?;
    let restore_target = match &request.command {
        maintenance_command::MaintenanceCommand::RestorePostgres { target_url_env, .. } => {
            if !target_url_env.starts_with("MADE_RESTORE_") {
                bail!("restore target must name a MADE_RESTORE_ host environment variable");
            }
            Some(std::env::var(target_url_env).context("restore target is not configured")?)
        }
        _ => None,
    };
    let cwd = std::env::current_dir()?;
    // The payload contains no principal selector. LocalHostPolicy identity comes
    // only from host configuration; the decision persists in its policy store.
    let store_identity = config
        .postgres_url
        .as_ref()
        .map(|url| AuthorizationTargetDigest::for_bytes(url.as_bytes()));
    let target = AuthorizationTargetDigest::for_bytes(&serde_json::to_vec(&(
        &request.command,
        cwd,
        &config.ceremony_store_path,
        &config.artifact_store_path,
        store_identity,
        restore_target
            .as_ref()
            .map(|url| AuthorizationTargetDigest::for_bytes(url.as_bytes())),
    ))?);
    if mode == "--describe" {
        println!(
            "{}",
            json!({"action": request.command.action(), "scope": {"kind":"global"}, "target_digest": target})
        );
        return Ok(());
    }
    let operation = authorization::authorize(&config, &request, target.clone()).await?;
    let evidence = operation.evidence().clone();
    let outcome = AuthorizationOperationScope::run(operation, async {
        if let maintenance_command::MaintenanceCommand::ReconcileExecution { receipt } =
            &request.command
        {
            return reconciliation::execute(&config, receipt).await;
        }
        if config.postgres_url.is_some() {
            return postgres_operations::execute(
                &config,
                &request.command,
                restore_target.as_deref(),
            )
            .await;
        }
        sqlite_operations::execute(&config, &request.command).await
    })
    .await;
    match outcome {
        Ok(result) => {
            println!(
                "{}",
                json!({"authorization":evidence,"target_digest":target,"result":result})
            );
            Ok(())
        }
        Err(error) => {
            println!(
                "{}",
                json!({"authorization":evidence,"target_digest":target,"completed":false})
            );
            Err(error)
        }
    }
}
