use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::Duration;

use made_adapters::clock::SystemClock;
use made_adapters::memory::{ForgetfulMemory, InMemoryCeremonyDefinitionRepository};
use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::workers::FileWorkerCapacityStore;
use made_adapters::yaml::FileSystemCeremonyDefinitionSource;
use made_app::services::SessionStream;
use made_app::usecases::{
    EnforceCeremonyDeadlinesUseCase, MountCeremonyDefinitionsUseCase,
    ResolveCeremonyDefinitionUseCase, StartCeremonyInput, StartCeremonyStepUseCase,
    StartCeremonyUseCase,
};
use made_app::workers::{
    CeremonyWorkerCapacity, CeremonyWorkerPolicy, ClaimCeremonyWorkInput, ClaimCeremonyWorkUseCase,
    RenewCeremonyStepLeaseUseCase, WorkerCapacityLimits,
};
use made_core::ports::NoopCeremonyEventSubscriber;
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyId, CeremonyInstancePageLimit, CeremonyName,
    CeremonyVersion, DurationMs, ExecutionConnectorId, ExecutionRecoveryPageLimit, LeaseOwnerId,
    MaxParallel, StepId,
};

const CHILD_MODE: &str = "MADE_CLAIM_CHILD";
const DEFINITION: &str = r#"
version: "1.0"
name: "multiprocess_worker"
description: "Real multiprocess worker claim fixture"
inputs: { required: [], optional: [] }
outputs: {}
states:
  - { id: OPEN, initial: true, terminal: false }
steps:
  - id: work
    state: OPEN
    handler: oci
    config: { connector: oci, provider: fixture }
roles:
  - id: WORKER
    allowed_actions: [work]
timeouts: { step_default: 30 }
retry_policies:
  default: { max_attempts: 3, backoff_seconds: 1 }
"#;

async fn definitions(path: &Path) -> Arc<InMemoryCeremonyDefinitionRepository> {
    let repository = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    MountCeremonyDefinitionsUseCase::new(
        Arc::new(FileSystemCeremonyDefinitionSource::from_directory(path).unwrap()),
        repository.clone(),
    )
    .execute()
    .await
    .unwrap();
    repository
}

fn stream(store: &Arc<SqliteCeremonyStore>) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ))
}

fn limits() -> WorkerCapacityLimits {
    WorkerCapacityLimits {
        global: CeremonyWorkerCapacity::new(2).unwrap(),
        per_root: CeremonyWorkerCapacity::new(2).unwrap(),
        per_connector: CeremonyWorkerCapacity::new(2).unwrap(),
        per_provider: CeremonyWorkerCapacity::new(2).unwrap(),
    }
}

#[tokio::test]
#[ignore = "subprocess helper"]
async fn claim_child() {
    if std::env::var(CHILD_MODE).ok().as_deref() != Some("1") {
        return;
    }
    let database = PathBuf::from(std::env::var("MADE_CLAIM_DATABASE").unwrap());
    let capacity_directory = PathBuf::from(std::env::var("MADE_CLAIM_CAPACITY").unwrap());
    let definition_directory = PathBuf::from(std::env::var("MADE_CLAIM_DEFINITIONS").unwrap());
    let result = PathBuf::from(std::env::var("MADE_CLAIM_RESULT").unwrap());
    let owner = LeaseOwnerId::new(std::env::var("MADE_CLAIM_OWNER").unwrap()).unwrap();
    let store = Arc::new(SqliteCeremonyStore::open(database).unwrap());
    let stream = stream(&store);
    let clock = Arc::new(SystemClock::new());
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        definitions(&definition_directory).await,
        store.clone(),
    ));
    let deadlines = Arc::new(EnforceCeremonyDeadlinesUseCase::new(
        resolver.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let policy = CeremonyWorkerPolicy::new(
        MaxParallel::new(1).unwrap(),
        ExecutionRecoveryPageLimit::new(10).unwrap(),
    );
    let claims = ClaimCeremonyWorkUseCase::new(
        store,
        stream.clone(),
        resolver.clone(),
        deadlines,
        Arc::new(StartCeremonyStepUseCase::new(
            resolver,
            stream.clone(),
            clock.clone(),
        )),
        clock,
        policy,
    )
    .with_shared_capacity(
        Arc::new(FileWorkerCapacityStore::new(capacity_directory, limits(), stream).unwrap()),
        ExecutionConnectorId::new("oci").unwrap(),
    );
    let input = ClaimCeremonyWorkInput::new(
        None,
        CeremonyInstancePageLimit::new(10).unwrap(),
        owner,
        DurationMs::from_millis(
            u64::try_from((500.0_f64 * timing_scale()).ceil() as u128).unwrap_or(u64::MAX),
        ),
        AuditActorKind::Service,
    );
    // The poll window must stay strictly inside the lease TTL the parent
    // hands over via MADE_CLAIM_LEASE_MS: with 8 attempts x 25 ms pause the
    // child polls for at most ~200 ms, well under 500 ms, so the scenario
    // "all three children are still polling while two claims are held" is
    // deterministic instead of a race with lease expiry.
    let attempts = 8_u32;
    let poll_pause = std::env::var("MADE_TEST_CLAIM_POLL_PAUSE_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20);
    let mut accepted = None;
    for _ in 0..attempts {
        let page = claims.execute(input.clone()).await.unwrap();
        if let Some(claim) = page.claims().first() {
            accepted = Some(claim.clone());
            break;
        }
        tokio::time::sleep(Duration::from_millis(poll_pause)).await;
    }
    let encoded = accepted.as_ref().map_or_else(
        || "none".to_owned(),
        |claim| {
            format!(
                "{}|{}|{}",
                claim.handler_request.instance_id(),
                claim.claim_fence.as_str(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            )
        },
    );
    std::fs::write(result, encoded).unwrap();
}

fn spawn_child(root: &Path, result: &Path, owner: &str) -> Child {
    let scale = timing_scale();
    // Keep the poll window (attempts x pause) strictly shorter than the lease
    // TTL even on a slow runner: the deterministic scenario is "two claims are
    // held when all three children are still polling". Both the window and the
    // TTL scale together, preserving that invariant under instrumentation.
    let lease_ms = (500.0_f64 * scale).ceil();
    Command::new(std::env::current_exe().unwrap())
        .arg("--ignored")
        .arg("--exact")
        .arg("claim_child")
        .arg("--test-threads=1")
        .env(CHILD_MODE, "1")
        .env("MADE_CLAIM_DATABASE", root.join("ceremonies.sqlite3"))
        .env("MADE_CLAIM_CAPACITY", root.join("capacity"))
        .env("MADE_CLAIM_DEFINITIONS", root.join("definitions"))
        .env("MADE_CLAIM_RESULT", result)
        .env("MADE_CLAIM_OWNER", owner)
        .env("MADE_CLAIM_LEASE_MS", lease_ms.to_string())
        .env(
            "MADE_TEST_CLAIM_POLL_PAUSE_MS",
            (25.0_f64 * scale).ceil().to_string(),
        )
        .spawn()
        .unwrap()
}

/// Wall-clock multiplier for fixed timing budgets in this suite. Under
/// llvm-cov instrumentation every child is several times slower; the
/// coverage job sets MADE_TEST_TIMING_SCALE so the budgets scale with it.
fn timing_scale() -> f64 {
    std::env::var("MADE_TEST_TIMING_SCALE")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|scale| *scale >= 1.0)
        .unwrap_or(1.0)
}

#[tokio::test]
async fn three_processes_compete_for_real_claims_and_recover_after_owner_death() {
    std::fs::create_dir_all("tmp").unwrap();
    let root = tempfile::tempdir_in("tmp").unwrap();
    let definition_directory = root.path().join("definitions");
    std::fs::create_dir_all(&definition_directory).unwrap();
    std::fs::write(definition_directory.join("worker.yaml"), DEFINITION).unwrap();
    let store =
        Arc::new(SqliteCeremonyStore::open(root.path().join("ceremonies.sqlite3")).unwrap());
    let stream = stream(&store);
    let clock = Arc::new(SystemClock::new());
    let repository = definitions(&definition_directory).await;
    let start = StartCeremonyUseCase::new(
        repository.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    );
    for index in 0..3 {
        start
            .execute(StartCeremonyInput::new(
                CeremonyId::new(format!("claim-{index}")).unwrap(),
                CeremonyName::new("multiprocess_worker").unwrap(),
                CeremonyVersion::v1(),
                CeremonyContext::empty(),
                "operator",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();
    }

    let mut children = Vec::new();
    let mut results = Vec::new();
    for index in 0..3 {
        let result = root.path().join(format!("claim-result-{index}"));
        children.push(spawn_child(root.path(), &result, &format!("owner-{index}")));
        results.push(result);
    }
    for (index, mut child) in children.into_iter().enumerate() {
        let status = child.wait().unwrap();
        assert!(status.success(), "claim child {index} exited with {status}");
    }
    let accepted = results
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .filter(|value| value != "none")
        .collect::<Vec<_>>();
    // The children poll for longer than the 500 ms lease TTL, so a third
    // process may legitimately claim after an earlier owner's lease has
    // expired. The capacity property is therefore not "exactly two accepted
    // claims" — that held only while the poll window was shorter than the
    // TTL — but "no more than two claims whose 500 ms leases overlap in
    // time". Assert that from the accept timestamps each child records.
    let lease_ms: u128 = 500;
    let mut sorted_times = accepted
        .iter()
        .map(|record| {
            record
                .split_once('|')
                .and_then(|(_, rest)| rest.split_once('|'))
                .map(|(_, at)| at)
                .expect("accepted record carries ceremony|fence|timestamp")
                .parse::<u128>()
                .expect("accept timestamp parses as millis")
        })
        .collect::<Vec<_>>();
    sorted_times.sort_unstable();
    let overlapping_pairs = sorted_times
        .windows(2)
        .filter(|pair| pair[1] < pair[0] + lease_ms)
        .count();
    assert!(
        overlapping_pairs <= 1,
        "shared capacity must never admit a third claim while two 500ms leases \
         still overlap; accepted={accepted:?}"
    );

    let (ceremony, fence) = accepted[0]
        .split_once('|')
        .and_then(|(ceremony, rest)| rest.split_once('|').map(|(fence, _)| (ceremony, fence)))
        .unwrap();
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(repository, store));
    let wrong_owner = RenewCeremonyStepLeaseUseCase::new(stream, resolver, clock)
        .execute(
            &CeremonyId::new(ceremony).unwrap(),
            &StepId::new("work").unwrap(),
            &made_core::value_objects::StepClaimFence::new(fence).unwrap(),
            &LeaseOwnerId::new("old-or-foreign-owner").unwrap(),
            DurationMs::from_millis(500),
        )
        .await;
    assert!(
        wrong_owner.is_err(),
        "an old owner must not renew another claim"
    );

    tokio::time::sleep(Duration::from_millis(650)).await;
    let recovered_result = root.path().join("recovered-result");
    let mut recovered = spawn_child(root.path(), &recovered_result, "replacement-owner");
    assert!(recovered.wait().unwrap().success());
    assert_ne!(
        std::fs::read_to_string(recovered_result).unwrap(),
        "none",
        "expired dead-owner claims and capacity must be reclaimable"
    );
}
