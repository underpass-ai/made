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
    let lease_ms = std::env::var("MADE_CLAIM_LEASE_MS")
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let input = ClaimCeremonyWorkInput::new(
        None,
        CeremonyInstancePageLimit::new(10).unwrap(),
        owner,
        DurationMs::from_millis(lease_ms),
        AuditActorKind::Service,
    );
    // A competing child polls 8 x 25 ms (scaled) and stops at its first
    // claim, so while two claims are held the third child is still polling.
    // Under a slow runner its window can outlast the lease TTL and it then
    // takes over an expired claim: the parent's assertions allow for that.
    // The replacement asks for more than one claim (MADE_CLAIM_TARGET) so it
    // must take over at least one dead owner's claim, not only fresh work.
    let target = std::env::var("MADE_CLAIM_TARGET")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_usize);
    let attempts = 8_u32;
    let poll_pause = std::env::var("MADE_TEST_CLAIM_POLL_PAUSE_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20);
    let mut accepted = Vec::new();
    let mut failures = Vec::new();
    for _ in 0..attempts {
        let before = unix_millis();
        let page = claims.execute(input.clone()).await.unwrap();
        let after = unix_millis();
        failures.extend(page.failures().iter().map(|failure| format!("{failure:?}")));
        for claim in page.claims() {
            accepted.push(format!(
                "claim|{}|{}|{}|{before}|{after}",
                claim.handler_request.instance_id(),
                claim.claim_fence.as_str(),
                claim.handler_request.attempt().get(),
            ));
        }
        if accepted.len() >= target {
            break;
        }
        tokio::time::sleep(Duration::from_millis(poll_pause)).await;
    }
    let lines = accepted
        .into_iter()
        .chain(
            failures
                .into_iter()
                .map(|failure| format!("failure|{failure}")),
        )
        .collect::<Vec<_>>();
    std::fs::write(result, lines.join("\n")).unwrap();
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

/// One claim a child accepted, as it recorded it. The lease started at some
/// instant between the two wall-clock readings around the claim call.
#[derive(Debug, Clone)]
struct AcceptedClaim {
    ceremony: String,
    fence: String,
    attempt: u32,
    not_before_ms: u128,
    not_after_ms: u128,
}

/// What one child process reported: its claims and every claim failure.
#[derive(Debug)]
struct ChildReport {
    claims: Vec<AcceptedClaim>,
    failures: Vec<String>,
}

fn read_report(path: &Path) -> ChildReport {
    let text = std::fs::read_to_string(path).unwrap();
    let mut report = ChildReport {
        claims: Vec::new(),
        failures: Vec::new(),
    };
    for line in text.lines() {
        if let Some(failure) = line.strip_prefix("failure|") {
            report.failures.push(failure.to_owned());
            continue;
        }
        let fields = line
            .strip_prefix("claim|")
            .expect("child report line is a claim or a failure")
            .split('|')
            .collect::<Vec<_>>();
        let [ceremony, fence, attempt, before, after] = fields.as_slice() else {
            panic!("claim line carries ceremony|fence|attempt|before|after: {line}");
        };
        report.claims.push(AcceptedClaim {
            ceremony: (*ceremony).to_owned(),
            fence: (*fence).to_owned(),
            attempt: attempt.parse().unwrap(),
            not_before_ms: before.parse().unwrap(),
            not_after_ms: after.parse().unwrap(),
        });
    }
    report
}

fn lease_ms() -> u64 {
    // Written as an integer: the child parses it back as u64.
    (500.0_f64 * timing_scale()).ceil() as u64
}

fn spawn_child(root: &Path, result: &Path, owner: &str, target: usize) -> Child {
    let scale = timing_scale();
    // The poll window (attempts x pause) and the lease TTL scale together, so
    // the window stays shorter than the TTL on an ordinary runner.
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
        .env("MADE_CLAIM_LEASE_MS", lease_ms().to_string())
        .env("MADE_CLAIM_TARGET", target.to_string())
        .env(
            "MADE_TEST_CLAIM_POLL_PAUSE_MS",
            (25.0_f64 * scale).ceil().to_string(),
        )
        .spawn()
        .unwrap()
}

/// A slow child may poll past the lease TTL and legitimately take over an
/// earlier owner's expired claim. The capacity property is therefore "no three
/// claims whose leases were live at the same instant". Three leases certainly
/// overlapped when the latest possible start among them is still inside the
/// lease of the earliest possible start.
fn assert_no_three_live_leases(accepted: &[AcceptedClaim], reports: &[ChildReport], lease: u128) {
    let certainly_overlapping = |triple: [&AcceptedClaim; 3]| {
        let latest_start = triple.iter().map(|claim| claim.not_after_ms).max();
        let earliest_start = triple.iter().map(|claim| claim.not_before_ms).min();
        latest_start.unwrap() < earliest_start.unwrap() + lease
    };
    let over_capacity = (0..accepted.len()).any(|first| {
        (first + 1..accepted.len()).any(|second| {
            (second + 1..accepted.len()).any(|third| {
                certainly_overlapping([&accepted[first], &accepted[second], &accepted[third]])
            })
        })
    });
    assert!(
        !accepted.is_empty() && !over_capacity,
        "shared capacity must admit work but never a third claim while two \
         {lease}ms leases are live; reports={reports:?}"
    );
}

/// Every child is dead. Wait until the last accepted lease has expired by the
/// wall clock (it started no later than its claim call returned), not a fixed
/// sleep that does not follow the scaled TTL. Returns that expiry instant.
async fn wait_until_every_lease_expired(accepted: &[AcceptedClaim], lease: u128) -> u128 {
    let last_expiry = accepted
        .iter()
        .map(|claim| claim.not_after_ms + lease)
        .max()
        .unwrap();
    while unix_millis() <= last_expiry {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    last_expiry
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
        children.push(spawn_child(
            root.path(),
            &result,
            &format!("owner-{index}"),
            1,
        ));
        results.push(result);
    }
    for (index, mut child) in children.into_iter().enumerate() {
        let status = child.wait().unwrap();
        assert!(status.success(), "claim child {index} exited with {status}");
    }
    let reports = results
        .iter()
        .map(|path| read_report(path))
        .collect::<Vec<_>>();
    let accepted = reports
        .iter()
        .flat_map(|report| report.claims.iter().cloned())
        .collect::<Vec<_>>();
    let lease = u128::from(lease_ms());
    assert_no_three_live_leases(&accepted, &reports, lease);

    let first = &accepted[0];
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(repository, store));
    let wrong_owner = RenewCeremonyStepLeaseUseCase::new(stream, resolver, clock)
        .execute(
            &CeremonyId::new(first.ceremony.as_str()).unwrap(),
            &StepId::new("work").unwrap(),
            &made_core::value_objects::StepClaimFence::new(first.fence.as_str()).unwrap(),
            &LeaseOwnerId::new("old-or-foreign-owner").unwrap(),
            DurationMs::from_millis(500),
        )
        .await;
    assert!(
        wrong_owner.is_err(),
        "an old owner must not renew another claim"
    );

    let last_expiry = wait_until_every_lease_expired(&accepted, lease).await;
    let replacement_started_at = unix_millis();
    let recovered_result = root.path().join("recovered-result");
    let mut recovered = spawn_child(root.path(), &recovered_result, "replacement-owner", 2);
    assert!(recovered.wait().unwrap().success());
    let replacement = read_report(&recovered_result);
    let dead_owner_ceremonies = accepted
        .iter()
        .map(|claim| claim.ceremony.as_str())
        .collect::<Vec<_>>();
    let taken_over = replacement
        .claims
        .iter()
        .filter(|claim| dead_owner_ceremonies.contains(&claim.ceremony.as_str()))
        .collect::<Vec<_>>();
    // With capacity 2 and three ceremonies, a replacement that holds two
    // claims holds at least one a dead owner left behind.
    assert!(
        replacement.claims.len() == 2
            && !taken_over.is_empty()
            && taken_over.iter().all(|claim| claim.attempt >= 2),
        "expired dead-owner claims and capacity must be reclaimable; \
         replacement started at {replacement_started_at} after last lease \
         expiry {last_expiry}; replacement={replacement:?}; \
         dead owners={reports:?}; capacity={:?}",
        std::fs::read_to_string(root.path().join("capacity").join("capacity.json"))
    );
}
