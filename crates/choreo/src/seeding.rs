//! Optional startup seeding of demo councils and agents.
//!
//! Controlled by `CHOREO_SEED_SPECIALTIES`: a comma-separated list
//! of specialty labels. For each label, the binary registers a
//! [`NoopAgent`] and a single-agent [`Council`] so the service is
//! immediately exercisable over gRPC / NATS.
//!
//! Seeding is **off by default**. The presence of the variable is
//! an opt-in operator choice. Real deployments wire agents through
//! adapter-specific channels (future slices: RegisterAgent RPC,
//! config-driven factories).

use choreo_adapters::noop::NoopAgent;
use choreo_core::entities::Council;
use choreo_core::error::DomainError;
use choreo_core::ports::{AgentRegistryPort, ClockPort, ContractRegistryPort, CouncilRegistryPort};
use choreo_core::value_objects::{AgentId, CouncilId, OutputContract, Specialty};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, warn};

/// Environment variable used to opt in to startup seeding.
pub const SEED_ENV_VAR: &str = "CHOREO_SEED_SPECIALTIES";

/// Environment variable used to opt in to startup contract seeding.
///
/// When set, points at a directory of `*.json` files; each file is
/// deserialized as an [`OutputContract`] and registered. Files that
/// fail to parse or register are logged at WARN and skipped — one bad
/// file doesn't take down the service.
pub const CONTRACT_DIR_ENV_VAR: &str = "CHOREO_CONTRACT_DIR";

#[derive(Debug, Error)]
pub enum SeedingError {
    #[error("seeding failed: {0}")]
    Domain(#[from] DomainError),
    #[error("contract seeding failed for {path}: {source}")]
    ContractIo {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Read the opt-in env var and apply seeding if present.
pub async fn apply_env_seeding(
    clock: &dyn ClockPort,
    agent_registry: &dyn AgentRegistryPort,
    council_registry: &dyn CouncilRegistryPort,
) -> Result<(), SeedingError> {
    let Ok(raw) = std::env::var(SEED_ENV_VAR) else {
        return Ok(());
    };
    let labels: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if labels.is_empty() {
        return Ok(());
    }
    info!(specialties = ?labels, "seeding demo councils");
    apply_seeding(clock, agent_registry, council_registry, &labels).await
}

/// Seed one `NoopAgent` and one single-agent council per specialty.
///
/// Split from [`apply_env_seeding`] so tests can drive seeding
/// without touching process env.
pub async fn apply_seeding(
    clock: &dyn ClockPort,
    agent_registry: &dyn AgentRegistryPort,
    council_registry: &dyn CouncilRegistryPort,
    specialties: &[&str],
) -> Result<(), SeedingError> {
    for label in specialties {
        let specialty = Specialty::new(*label)?;
        let agent_id = AgentId::new(format!("seed-{label}-0"))?;
        let agent = Arc::new(NoopAgent::new(agent_id.clone(), specialty.clone()));
        // If the agent is already registered (re-seeding) swallow the
        // AlreadyExists error so restarts stay idempotent.
        match agent_registry.register(agent).await {
            Ok(()) | Err(DomainError::AlreadyExists { .. }) => {}
            Err(err) => return Err(err.into()),
        }
        let council_id = CouncilId::new(format!("seed-{label}"))?;
        let council = Council::new(council_id, specialty.clone(), vec![agent_id], clock.now())?;
        match council_registry.register(council).await {
            Ok(()) | Err(DomainError::AlreadyExists { .. }) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

/// Read `CHOREO_CONTRACT_DIR` and register every `*.json` contract it
/// contains. Idempotent: contracts that already exist are skipped at
/// WARN level, contracts that fail to parse are skipped at WARN level
/// without aborting startup. The directory itself missing returns
/// `Ok(())` — seeding is fully opt-in.
pub async fn apply_contract_seeding(
    contracts: &dyn ContractRegistryPort,
) -> Result<(), SeedingError> {
    let Ok(raw) = std::env::var(CONTRACT_DIR_ENV_VAR) else {
        return Ok(());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    apply_contract_seeding_from_dir(contracts, Path::new(trimmed)).await
}

/// Seed every `*.json` contract under `dir`. Split from
/// [`apply_contract_seeding`] so tests can drive the path without
/// racing the process-wide env table.
pub async fn apply_contract_seeding_from_dir(
    contracts: &dyn ContractRegistryPort,
    dir: &Path,
) -> Result<(), SeedingError> {
    if !dir.exists() {
        warn!(
            dir = %dir.display(),
            "contract seeding directory does not exist; skipping"
        );
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|err| SeedingError::ContractIo {
        path: dir.display().to_string(),
        source: err,
    })?;
    let mut json_paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json")
        })
        .collect();
    json_paths.sort();
    info!(
        dir = %dir.display(),
        count = json_paths.len(),
        "seeding output contracts"
    );
    for path in json_paths {
        let label = path.display().to_string();
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) => {
                warn!(path = %label, error = %err, "skipping contract: cannot read file");
                continue;
            }
        };
        let contract: OutputContract = match serde_json::from_str(&raw) {
            Ok(c) => c,
            Err(err) => {
                warn!(path = %label, error = %err, "skipping contract: invalid JSON");
                continue;
            }
        };
        let id = contract.contract_id().to_owned();
        match contracts.register(contract).await {
            Ok(()) => info!(path = %label, contract_id = %id, "contract seeded"),
            Err(DomainError::AlreadyExists { .. }) => {
                warn!(path = %label, contract_id = %id, "contract already registered; skipping");
            }
            Err(err) => {
                warn!(path = %label, contract_id = %id, error = %err, "contract registration failed");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use choreo_adapters::clock::SystemClock;
    use choreo_adapters::memory::{
        InMemoryAgentRegistry, InMemoryContractRegistry, InMemoryCouncilRegistry,
    };
    use choreo_core::ports::{AgentResolverPort, CouncilRegistryPort};

    #[tokio::test]
    async fn seeding_is_idempotent() {
        let clock = SystemClock::new();
        let agents = InMemoryAgentRegistry::new();
        let councils = InMemoryCouncilRegistry::new();

        apply_seeding(&clock, &agents, &councils, &["triage", "reviewer"])
            .await
            .unwrap();
        assert_eq!(councils.len().await, 2);
        assert_eq!(agents.len().await, 2);

        // Second pass must not fail and must not duplicate.
        apply_seeding(&clock, &agents, &councils, &["triage", "reviewer"])
            .await
            .unwrap();
        assert_eq!(councils.len().await, 2);
        assert_eq!(agents.len().await, 2);
    }

    #[tokio::test]
    async fn seeded_council_is_resolvable_and_matches_specialty() {
        let clock = SystemClock::new();
        let agents = InMemoryAgentRegistry::new();
        let councils = InMemoryCouncilRegistry::new();
        apply_seeding(&clock, &agents, &councils, &["triage"])
            .await
            .unwrap();

        let triage = Specialty::new("triage").unwrap();
        let council = councils.get(&triage).await.unwrap();
        assert_eq!(council.specialty(), &triage);
        assert_eq!(council.size(), 1);

        // The agent referenced by the council resolves.
        let id = council.agents().iter().next().unwrap();
        let resolved = agents.resolve(id).await.unwrap();
        assert_eq!(resolved.specialty(), &triage);
    }

    #[tokio::test]
    async fn invalid_specialty_surfaces_as_domain_error() {
        let clock = SystemClock::new();
        let agents = InMemoryAgentRegistry::new();
        let councils = InMemoryCouncilRegistry::new();
        let err = apply_seeding(&clock, &agents, &councils, &["   "])
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            SeedingError::Domain(DomainError::EmptyField { .. })
        ));
    }

    #[tokio::test]
    async fn empty_label_list_is_a_noop() {
        let clock = SystemClock::new();
        let agents = InMemoryAgentRegistry::new();
        let councils = InMemoryCouncilRegistry::new();
        apply_seeding(&clock, &agents, &councils, &[])
            .await
            .unwrap();
        assert!(agents.is_empty().await);
        assert!(councils.is_empty().await);
    }

    #[tokio::test]
    async fn contract_seeding_from_missing_dir_is_a_noop() {
        let contracts = InMemoryContractRegistry::new();
        // Build a path that we know does not exist by appending a
        // suffix to a freshly-minted unique name. Using `.exists()`
        // would tolerate races; constructing it never to exist keeps
        // the assertion deterministic.
        let nope = std::env::temp_dir().join(format!(
            "choreo-contract-seeding-missing-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        apply_contract_seeding_from_dir(&contracts, &nope)
            .await
            .unwrap();
        assert!(contracts.is_empty().await);
    }

    #[tokio::test]
    async fn contract_seeding_registers_every_json_file() {
        let tmp = tempdir_with_contracts(&[
            (
                "first.json",
                r#"{"contract_id":"first","format":"JsonObject","fields":{}}"#,
            ),
            (
                "second.json",
                r#"{"contract_id":"second","format":"JsonObject","fields":{}}"#,
            ),
            ("ignored.txt", "not json"),
        ]);

        let contracts = InMemoryContractRegistry::new();
        apply_contract_seeding_from_dir(&contracts, tmp.path())
            .await
            .unwrap();
        assert_eq!(contracts.len().await, 2);
        assert!(contracts.contains("first").await.unwrap());
        assert!(contracts.contains("second").await.unwrap());
    }

    #[tokio::test]
    async fn contract_seeding_skips_invalid_json_without_failing() {
        let tmp = tempdir_with_contracts(&[
            ("broken.json", "{this is not json"),
            (
                "good.json",
                r#"{"contract_id":"good","format":"JsonObject","fields":{}}"#,
            ),
        ]);

        let contracts = InMemoryContractRegistry::new();
        apply_contract_seeding_from_dir(&contracts, tmp.path())
            .await
            .unwrap();
        assert!(contracts.contains("good").await.unwrap());
        assert_eq!(contracts.len().await, 1);
    }

    struct TempDirGuard {
        path: std::path::PathBuf,
    }
    impl TempDirGuard {
        fn path(&self) -> &Path {
            &self.path
        }
    }
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Create a unique temp dir, write each `(name, content)` into it,
    /// and return a guard that cleans the directory on drop. Kept
    /// here to avoid adding a `tempfile` dev-dep just for two tests.
    fn tempdir_with_contracts(files: &[(&str, &str)]) -> TempDirGuard {
        let unique = format!(
            "choreo-contract-seeding-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).unwrap();
        for (name, content) in files {
            std::fs::write(path.join(name), content).unwrap();
        }
        TempDirGuard { path }
    }
}
