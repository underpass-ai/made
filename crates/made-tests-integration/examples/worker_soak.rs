//! Persistent integration soak for the public recoverable worker host.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use made_adapters::clock::SystemClock;
use made_adapters::execution::RepositoryScriptExecutionConnector;
use made_adapters::memory::{
    ForgetfulMemory, InMemoryCeremonyDefinitionPublications, InMemoryCeremonyDefinitionRepository,
};
use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::yaml::FileSystemCeremonyDefinitionSource;
use made_app::services::SessionStream;
use made_app::usecases::{
    EnforceCeremonyDeadlinesUseCase, MountCeremonyDefinitionsUseCase,
    ResolveCeremonyDefinitionUseCase, StartCeremonyInput, StartCeremonyStepUseCase,
    StartCeremonyUseCase,
};
use made_app::workers::{
    CeremonyWorkerDriver, CeremonyWorkerHost, CeremonyWorkerPolicy, CeremonyWorkerStopToken,
    ClaimCeremonyWorkInput, ClaimCeremonyWorkUseCase, CompleteExecutionReceiptUseCase,
    ExecuteCeremonyOperationUseCase, InspectExecutionRecoveryUseCase,
    RecoverExecutionIntentUseCase, RecoverableCeremonyWorker, RecoverableCeremonyWorkerOutcome,
};
use made_core::ports::{
    CeremonyExecutionConnectorPort, ExecutionReceiptStorePort, NoopCeremonyEventSubscriber,
};
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion, DurationMs,
    ExecutionConnectorId, ExecutionRecoveryPageLimit, LeaseOwnerId, MaxParallel,
};
use serde_json::json;

#[path = "worker_soak/composition.rs"]
mod composition;
#[path = "worker_soak/drive.rs"]
mod drive;
#[path = "worker_soak/verification.rs"]
mod verification;

const DEFINITION: &str = r#"
version: "1.0"
name: "worker_soak"
description: "Persistent recoverable worker soak"
inputs:
  required: []
  optional: []
outputs: {}
states:
  - id: OPEN
    initial: true
    terminal: false
steps:
  - id: work
    state: OPEN
    handler: repository_script
    config: {}
roles:
  - id: WORKER
    allowed_actions:
      - work
retry_policies:
  default:
    max_attempts: 2
    backoff_seconds: 1
"#;

#[derive(Debug)]
struct Config {
    store: PathBuf,
    repository: PathBuf,
    script: String,
    duration: Duration,
    workers: u16,
}

impl Config {
    fn parse() -> Result<Self, String> {
        let mut store = None;
        let mut repository = None;
        let mut script = "scripts/soak/repository-worker.py".to_owned();
        let mut duration = None;
        let mut workers = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value after `{argument}`"))?;
            match argument.as_str() {
                "--store" => store = Some(PathBuf::from(value)),
                "--repository" => repository = Some(PathBuf::from(value)),
                "--script" => script = value,
                "--duration-seconds" => {
                    duration = Some(Duration::from_secs(parse_positive(&value, "duration")?));
                }
                "--workers" => {
                    let parsed = parse_positive(&value, "workers")?;
                    workers =
                        Some(u16::try_from(parsed).map_err(|_| "workers exceeds u16".to_owned())?);
                }
                _ => return Err(format!("unknown argument `{argument}`")),
            }
        }
        Ok(Self {
            store: store.ok_or_else(|| "--store is required".to_owned())?,
            repository: repository.ok_or_else(|| "--repository is required".to_owned())?,
            script,
            duration: duration.ok_or_else(|| "--duration-seconds is required".to_owned())?,
            workers: workers.ok_or_else(|| "--workers is required".to_owned())?,
        })
    }
}

fn parse_positive(raw: &str, field: &str) -> Result<u64, String> {
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| format!("{field} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{field} must be positive"));
    }
    Ok(parsed)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::parse().map_err(std::io::Error::other)?;
    if config.workers > ExecutionRecoveryPageLimit::MAX {
        return Err(format!("workers must be <= {}", ExecutionRecoveryPageLimit::MAX).into());
    }
    let parent = config
        .store
        .parent()
        .ok_or("store path must have a parent")?;
    std::fs::create_dir_all(parent)?;
    let operation_root = config.store.with_extension("worker-operations");
    let definition_root = config.store.with_extension("worker-definitions");
    std::fs::create_dir_all(&definition_root)?;
    std::fs::write(definition_root.join("worker_soak.yaml"), DEFINITION)?;

    let harness = composition::compose(&config, &operation_root, &definition_root).await?;

    let started_at = Instant::now();
    let deadline = started_at + config.duration;
    let run_id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let invocations_before = invocation_ids(&operation_root)?.len();
    let drive::Counts {
        seeded,
        completed,
        reconciliation_required,
        claim_failures,
        item_failures,
        recovered,
    } = drive::run(&config, &harness, deadline, run_id).await?;

    let invocations = invocation_ids(&operation_root)?;
    let unique = invocations.iter().collect::<HashSet<_>>().len();
    let duplicates = invocations.len().saturating_sub(unique);
    let persisted_completed = verification::verify(
        &harness.store,
        &harness.stream,
        &harness.inspector,
        &operation_root,
        "soak-",
    )
    .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "store": config.store,
            "operation_root": operation_root,
            "duration_seconds": started_at.elapsed().as_secs_f64(),
            "workers": config.workers,
            "seeded": seeded,
            "completed": completed,
            "persisted_completed_and_verified": persisted_completed,
            "recovered": recovered,
            "reconciliation_required": reconciliation_required,
            "claim_failures": claim_failures,
            "item_failures": item_failures,
            "invocations_before": invocations_before,
            "invocations_total": invocations.len(),
            "duplicate_operation_invocations": duplicates,
        }))?
    );
    if duplicates > 0
        || claim_failures > 0
        || item_failures > 0
        || reconciliation_required > 0
        || seeded == 0
        || persisted_completed != invocations.len() as u64
        || invocations.len().saturating_sub(invocations_before) as u64 != seeded
    {
        return Err("worker soak observed incomplete work, duplicate execution or failures".into());
    }
    Ok(())
}

fn count_outcomes(
    outcomes: &[RecoverableCeremonyWorkerOutcome],
    completed: &mut u64,
    reconciliation_required: &mut u64,
) {
    for outcome in outcomes {
        match outcome {
            RecoverableCeremonyWorkerOutcome::Completed { .. } => *completed += 1,
            RecoverableCeremonyWorkerOutcome::ReconciliationRequired(_) => {
                *reconciliation_required += 1;
            }
        }
    }
}

fn invocation_ids(operation_root: &Path) -> Result<Vec<String>, std::io::Error> {
    let path = operation_root.join("invocations.log");
    match std::fs::read_to_string(path) {
        Ok(raw) => Ok(raw.lines().map(ToOwned::to_owned).collect()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}
