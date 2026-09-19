use std::path::PathBuf;
use std::time::Duration;

use made_app::workers::{
    CeremonyWorkerAdmissionPolicy, CeremonyWorkerCapacity, CeremonyWorkerCost,
    CeremonyWorkerHostPolicy, CeremonyWorkerPolicy, CeremonyWorkerPolicyVersion,
    CeremonyWorkerPriority, CeremonyWorkerSchedulerPolicy, CeremonyWorkerWeight,
    WorkerCapacityLimits,
};
use made_core::value_objects::{
    CeremonyInstancePageLimit, DurationMs, ExecutionRecoveryPageLimit, LeaseOwnerId, MaxParallel,
    PrincipalId,
};
use made_core::DomainError;

use super::WorkerConnectorKind;

#[derive(Debug, Clone)]
pub(crate) struct WorkerDaemonConfig {
    pub(crate) connector: WorkerConnectorKind,
    pub(crate) owner: LeaseOwnerId,
    pub(crate) principal: PrincipalId,
    pub(crate) lease_ttl: DurationMs,
    pub(crate) heartbeat: Duration,
    pub(crate) claim_page: CeremonyInstancePageLimit,
    pub(crate) worker_policy: CeremonyWorkerPolicy,
    pub(crate) admission_policy: CeremonyWorkerAdmissionPolicy,
    pub(crate) root_policies_json: Option<String>,
    pub(crate) admission_log: PathBuf,
    pub(crate) host_policy: CeremonyWorkerHostPolicy,
    pub(crate) capacity_limits: WorkerCapacityLimits,
    pub(crate) capacity_directory: PathBuf,
    pub(crate) operation_root: PathBuf,
    pub(crate) connector_timeout: Duration,
    pub(crate) oci_image: Option<String>,
    pub(crate) oci_workspace: Option<PathBuf>,
    pub(crate) oci_network: String,
    pub(crate) oci_uid: u32,
    pub(crate) oci_cpus: f64,
    pub(crate) oci_memory_bytes: u64,
    pub(crate) oci_pids: u32,
    pub(crate) oci_max_output_bytes: usize,
    pub(crate) git_repository: Option<PathBuf>,
    pub(crate) git_scratch: Option<PathBuf>,
    pub(crate) http_base: Option<String>,
}

impl WorkerDaemonConfig {
    pub(crate) fn from_env() -> Result<Option<Self>, DomainError> {
        if !env_bool("MADE_WORKER_ENABLED", false)? {
            return Ok(None);
        }
        let connector = WorkerConnectorKind::parse(&required("MADE_WORKER_CONNECTOR")?)?;
        let max_parallel = MaxParallel::new(env_parse("MADE_WORKER_MAX_PARALLEL", 3_u8)?)?;
        let scheduler_capacity = capacity(
            "MADE_WORKER_SCHEDULER_CAPACITY",
            u32::from(max_parallel.get()),
        )?;
        let lease_ttl = DurationMs::from_millis(env_parse("MADE_WORKER_LEASE_TTL_MS", 30_000_u64)?);
        let heartbeat_ms = env_parse("MADE_WORKER_HEARTBEAT_MS", 10_000_u64)?;
        if heartbeat_ms == 0 || heartbeat_ms >= lease_ttl.get() {
            return Err(invalid(
                "worker heartbeat must be positive and shorter than lease TTL",
            ));
        }
        let operation_root = PathBuf::from(required("MADE_WORKER_OPERATION_ROOT")?);
        let capacity_directory = PathBuf::from(required("MADE_WORKER_CAPACITY_DIRECTORY")?);
        let admission_log = optional("MADE_WORKER_ADMISSION_LOG_PATH").map_or_else(
            || capacity_directory.join("admission-decisions.jsonl"),
            PathBuf::from,
        );
        Ok(Some(Self {
            connector,
            owner: LeaseOwnerId::new(required("MADE_WORKER_OWNER_ID")?)?,
            principal: PrincipalId::new(required("MADE_WORKER_PRINCIPAL_ID")?)?,
            lease_ttl,
            heartbeat: Duration::from_millis(heartbeat_ms),
            claim_page: CeremonyInstancePageLimit::new(env_parse(
                "MADE_WORKER_CLAIM_PAGE_LIMIT",
                50_u16,
            )?)?,
            worker_policy: CeremonyWorkerPolicy::new(
                max_parallel,
                ExecutionRecoveryPageLimit::new(env_parse(
                    "MADE_WORKER_RECOVERY_PAGE_LIMIT",
                    100_u16,
                )?)?,
            ),
            admission_policy: CeremonyWorkerAdmissionPolicy::new(
                CeremonyWorkerSchedulerPolicy::new(
                    max_parallel,
                    scheduler_capacity,
                    CeremonyWorkerPolicyVersion::new(env_parse(
                        "MADE_WORKER_POLICY_VERSION",
                        1_u64,
                    )?)?,
                ),
                CeremonyWorkerPriority::new(env_parse("MADE_WORKER_PRIORITY", 0_u16)?)?,
                CeremonyWorkerWeight::new(env_parse("MADE_WORKER_WEIGHT", 1_u32)?)?,
                CeremonyWorkerCost::new(env_parse("MADE_WORKER_COST", 1_u32)?)?,
                capacity("MADE_WORKER_REQUESTED_CAPACITY", 1)?,
            ),
            root_policies_json: optional("MADE_WORKER_ROOT_POLICIES_JSON"),
            admission_log,
            host_policy: CeremonyWorkerHostPolicy::new(
                env_parse("MADE_WORKER_MAX_PAGES_PER_TURN", 4_u16)?,
                DurationMs::from_millis(env_parse("MADE_WORKER_INITIAL_BACKOFF_MS", 100_u64)?),
                DurationMs::from_millis(env_parse("MADE_WORKER_MAX_BACKOFF_MS", 2_000_u64)?),
            )?,
            capacity_limits: WorkerCapacityLimits {
                global: capacity("MADE_WORKER_CAPACITY_GLOBAL", u32::from(max_parallel.get()))?,
                per_root: capacity("MADE_WORKER_CAPACITY_PER_ROOT", 1)?,
                per_connector: capacity(
                    "MADE_WORKER_CAPACITY_PER_CONNECTOR",
                    u32::from(max_parallel.get()),
                )?,
                per_provider: capacity("MADE_WORKER_CAPACITY_PER_PROVIDER", 1)?,
            },
            capacity_directory,
            operation_root,
            connector_timeout: Duration::from_millis(env_parse(
                "MADE_WORKER_CONNECTOR_TIMEOUT_MS",
                300_000_u64,
            )?),
            oci_image: optional("MADE_WORKER_OCI_IMAGE"),
            oci_workspace: optional("MADE_WORKER_OCI_WORKSPACE").map(PathBuf::from),
            oci_network: optional("MADE_WORKER_OCI_NETWORK").unwrap_or_else(|| "none".to_owned()),
            oci_uid: env_parse("MADE_WORKER_OCI_UID", 65_532_u32)?,
            oci_cpus: env_parse("MADE_WORKER_OCI_CPUS", 1.0_f64)?,
            oci_memory_bytes: env_parse("MADE_WORKER_OCI_MEMORY_BYTES", 536_870_912_u64)?,
            oci_pids: env_parse("MADE_WORKER_OCI_PIDS", 128_u32)?,
            oci_max_output_bytes: env_parse("MADE_WORKER_OCI_MAX_OUTPUT_BYTES", 1_048_576_usize)?,
            git_repository: optional("MADE_WORKER_GIT_REPOSITORY").map(PathBuf::from),
            git_scratch: optional("MADE_WORKER_GIT_SCRATCH").map(PathBuf::from),
            http_base: optional("MADE_WORKER_HTTP_BASE"),
        }))
    }
}

fn capacity(name: &'static str, default: u32) -> Result<CeremonyWorkerCapacity, DomainError> {
    CeremonyWorkerCapacity::new(env_parse(name, default)?)
}

fn optional(name: &'static str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn required(name: &'static str) -> Result<String, DomainError> {
    optional(name).ok_or_else(|| invalid(format!("{name} is required when workers are enabled")))
}

fn env_bool(name: &'static str, default: bool) -> Result<bool, DomainError> {
    match optional(name).as_deref() {
        None => Ok(default),
        Some("true" | "1") => Ok(true),
        Some("false" | "0") => Ok(false),
        Some(_) => Err(invalid(format!("{name} must be true or false"))),
    }
}

fn env_parse<T>(name: &'static str, default: T) -> Result<T, DomainError>
where
    T: std::str::FromStr,
{
    optional(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|_| invalid(format!("{name} has an invalid value")))
    })
}

fn invalid(reason: impl Into<String>) -> DomainError {
    DomainError::InvalidDocument {
        reason: reason.into(),
    }
}
