#![cfg(unix)]

use std::collections::HashMap;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use made_adapters::clock::SystemClock;
use made_adapters::memory::{ForgetfulMemory, InMemoryCeremonyDefinitionRepository};
use made_adapters::sqlite::{
    SqliteAuthorizationPolicyStore, SqliteBudgetLedgerStore, SqliteCeremonyStore,
};
use made_adapters::yaml::FileSystemCeremonyDefinitionSource;
use made_app::authorization::AuthorizationPolicyAdministrationService;
use made_app::budgets::{
    BudgetLedgerService, StartBudgetedCeremonyInput, StartBudgetedCeremonyUseCase,
};
use made_app::services::SessionStream;
use made_app::usecases::{
    CancelCeremonyInput, CancelCeremonyUseCase, MountCeremonyDefinitionsUseCase,
    PauseCeremonyInput, PauseCeremonyUseCase, PublishCeremonyDefinitionUseCase,
    ResolveCeremonyDefinitionUseCase, StartCeremonyInput,
};
use made_core::ports::{
    BudgetLedgerStorePort, CeremonyDefinitionRepositoryPort, ExecutionReceiptStorePort,
    NoopCeremonyEventSubscriber,
};
use made_core::value_objects::{
    AuditActorKind, AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationRevocationReason, AuthorizationScope, BudgetMeasurement, BudgetQuantities,
    BudgetTokenCount, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion, CostMicros,
    DelegationDepth, ExecutionDuration, LifecycleReason, PrincipalId, PrincipalKind, StepId,
    StepOutput, StepResult, StepStatus, ToolCallCount,
};
use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyUsagePurpose};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

const DEFINITION: &str = r#"
version: "1.0"
name: "http_worker_acceptance"
description: "Production worker composition acceptance"
inputs: { required: [], optional: [] }
outputs: {}
states:
  - { id: OPEN, initial: true, terminal: false }
steps:
  - id: work
    state: OPEN
    handler: http
    config:
      connector: http
      provider: acceptance
      budget_policy_version: 1
      estimated_duration_micros: 1000
      estimated_tokens: 1
      estimated_cost_micros: 0
      estimated_tool_calls: 1
      action: accept
roles:
  - id: WORKER
    allowed_actions: [work]
timeouts: { step_default: 30 }
retry_policies:
  default: { max_attempts: 3, backoff_seconds: 1 }
"#;

const FAIL_CLOSED_BUDGET_DEFINITION: &str = r#"
version: "1.0"
name: "http_worker_budget_fail_closed"
description: "Installed worker budget policy acceptance"
inputs: { required: [], optional: [] }
outputs: {}
states:
  - { id: OPEN, initial: true, terminal: false }
steps:
  - id: valid
    state: OPEN
    handler: http
    config:
      connector: http
      provider: acceptance
      budget_policy_version: 1
      estimated_duration_micros: 1000
      estimated_tokens: 1
      estimated_cost_micros: 0
      estimated_tool_calls: 1
      action: valid
  - id: missing
    state: OPEN
    handler: http
    config:
      connector: http
      provider: acceptance
      budget_policy_version: 1
      estimated_duration_micros: 1000
      estimated_cost_micros: 0
      estimated_tool_calls: 1
      action: missing
  - id: unknown
    state: OPEN
    handler: http
    config:
      connector: http
      provider: acceptance
      budget_policy_version: 1
      estimated_duration_micros: 1000
      estimated_tokens: unknown
      estimated_cost_micros: 0
      estimated_tool_calls: 1
      action: unknown
  - id: stale
    state: OPEN
    handler: http
    config:
      connector: http
      provider: acceptance
      budget_policy_version: 2
      estimated_duration_micros: 1000
      estimated_tokens: 1
      estimated_cost_micros: 0
      estimated_tool_calls: 1
      action: stale
  - id: over_ceiling
    state: OPEN
    handler: http
    config:
      connector: http
      provider: acceptance
      budget_policy_version: 1
      estimated_duration_micros: 1001
      estimated_tokens: 1
      estimated_cost_micros: 0
      estimated_tool_calls: 1
      action: over_ceiling
roles:
  - id: WORKER
    allowed_actions: [valid, missing, unknown, stale, over_ceiling]
timeouts: { step_default: 30 }
retry_policies:
  default: { max_attempts: 3, backoff_seconds: 1 }
"#;

const COMPOSED_LIFECYCLE_DEFINITION: &str = r#"
version: "1.0"
name: "http_worker_composed_lifecycle"
description: "One operation crossing pause, cancellation, restart, and competition"
inputs: { required: [], optional: [] }
outputs: {}
states:
  - { id: OPEN, initial: true, terminal: false }
steps:
  - id: work
    state: OPEN
    handler: http
    config:
      connector: http
      provider: acceptance
      budget_policy_version: 1
      estimated_duration_micros: 1000
      estimated_tokens: 1
      estimated_cost_micros: 0
      estimated_tool_calls: 1
      action: work
  - id: held_back
    state: OPEN
    handler: http
    config:
      connector: http
      provider: acceptance
      budget_policy_version: 1
      estimated_duration_micros: 1000
      estimated_tokens: 1
      estimated_cost_micros: 0
      estimated_tool_calls: 1
      action: held_back
roles:
  - id: WORKER
    allowed_actions: [work, held_back]
timeouts: { step_default: 4 }
retry_policies:
  default: { max_attempts: 3, backoff_seconds: 1 }
"#;

#[derive(Clone, Default)]
struct AcceptanceState {
    operations: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    gets: Arc<Mutex<usize>>,
    puts: Arc<Mutex<usize>>,
    response_delay: Arc<Mutex<Duration>>,
    response_gate: Arc<Mutex<Option<Arc<tokio::sync::Notify>>>>,
}

async fn get_operation(
    State(state): State<AcceptanceState>,
    AxumPath(key): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    *state.gets.lock().unwrap() += 1;
    state
        .operations
        .lock()
        .unwrap()
        .get(&key)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn put_operation(
    State(state): State<AcceptanceState>,
    AxumPath(key): AxumPath<String>,
    Json(submitted): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let response = serde_json::json!({
        "operation_id": submitted["operation_id"],
        "request_digest": submitted["request_digest"],
        "producer_claim_fence": submitted["producer_claim_fence"],
        "result": StepResult::completed(StepOutput::empty()).unwrap(),
        "observed_at": "1970-01-01T00:00:00Z",
    });
    *state.puts.lock().unwrap() += 1;
    state
        .operations
        .lock()
        .unwrap()
        .insert(key, response.clone());
    let response_gate = state.response_gate.lock().unwrap().clone();
    if let Some(response_gate) = response_gate {
        response_gate.notified().await;
    } else {
        let delay = *state.response_delay.lock().unwrap();
        tokio::time::sleep(delay).await;
    }
    Json(response)
}

struct TlsMaterial {
    ca_pem: Vec<u8>,
    server_cert_pem: Vec<u8>,
    server_key_pem: Vec<u8>,
    client_der: Vec<u8>,
}

struct ServiceProcess {
    child: Child,
    grpc_port: u16,
    http_port: u16,
}

impl Drop for ServiceProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn mint_tls() -> TlsMaterial {
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();
    let mut server_params = CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
    let mut server_name = DistinguishedName::new();
    server_name.push(DnType::CommonName, "localhost");
    server_params.distinguished_name = server_name;
    let server_key = rcgen::KeyPair::generate().unwrap();
    let server_cert = server_params
        .signed_by(&server_key, &ca_cert, &ca_key)
        .unwrap();
    let client_key = rcgen::KeyPair::generate().unwrap();
    let client_cert = CertificateParams::new(Vec::<String>::new())
        .unwrap()
        .signed_by(&client_key, &ca_cert, &ca_key)
        .unwrap();
    TlsMaterial {
        ca_pem: ca_cert.pem().into_bytes(),
        server_cert_pem: server_cert.pem().into_bytes(),
        server_key_pem: server_key.serialize_pem().into_bytes(),
        client_der: client_cert.der().to_vec(),
    }
}

fn stream(store: &Arc<SqliteCeremonyStore>) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ))
}

async fn seed(root: &Path, database: &Path, ceremony: &CeremonyId) {
    seed_with_definition(
        root,
        database,
        ceremony,
        DEFINITION,
        "http_worker_acceptance",
    )
    .await;
}

async fn seed_with_definition(
    root: &Path,
    database: &Path,
    ceremony: &CeremonyId,
    definition: &str,
    definition_name: &str,
) {
    let definitions_path = root.join("definitions");
    std::fs::create_dir_all(&definitions_path).unwrap();
    std::fs::write(definitions_path.join("worker.yaml"), definition).unwrap();
    let definitions = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    MountCeremonyDefinitionsUseCase::new(
        Arc::new(FileSystemCeremonyDefinitionSource::from_directory(&definitions_path).unwrap()),
        definitions.clone(),
    )
    .execute()
    .await
    .unwrap();
    let store = Arc::new(SqliteCeremonyStore::open(database).unwrap());
    let definition = definitions
        .get(
            &CeremonyName::new(definition_name).unwrap(),
            &CeremonyVersion::v1(),
        )
        .await
        .unwrap();
    PublishCeremonyDefinitionUseCase::new(store.clone())
        .execute(definition)
        .await
        .unwrap();
    let clock = Arc::new(SystemClock::new());
    StartBudgetedCeremonyUseCase::new(
        store.clone(),
        stream(&store),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
        BudgetLedgerService::new(
            Arc::new(SqliteBudgetLedgerStore::open(database).unwrap()),
            clock.clone(),
        ),
    )
    .execute(StartBudgetedCeremonyInput::new(
        StartCeremonyInput::new(
            ceremony.clone(),
            CeremonyName::new(definition_name).unwrap(),
            CeremonyVersion::v1(),
            CeremonyContext::empty(),
            "acceptance-owner",
            AuditActorKind::Service,
        ),
        made_core::value_objects::BudgetLimits::new(
            BudgetQuantities::new(
                ExecutionDuration::from_micros(10_000),
                BudgetTokenCount::new(10),
                CostMicros::new(0),
                ToolCallCount::new(10),
            ),
            None,
        )
        .unwrap(),
    ))
    .await
    .unwrap();

    let policies = Arc::new(SqliteAuthorizationPolicyStore::open(database).unwrap());
    let policy = AuthorizationPolicyId::new("worker-acceptance-policy").unwrap();
    let owner = AuthenticatedPrincipal::new(
        PrincipalId::new("acceptance-owner").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    let administration = AuthorizationPolicyAdministrationService::new(policy, policies, clock);
    administration.open(owner.clone(), vec![]).await.unwrap();
    administration
        .issue(
            &owner,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("worker-session-grant").unwrap(),
                PrincipalId::new("acceptance-worker").unwrap(),
                [
                    AuthorizationAction::EnforceCeremonyDeadlines,
                    AuthorizationAction::ClaimCeremonyStep,
                ],
                AuthorizationScope::Ceremony {
                    ceremony_id: ceremony.clone(),
                },
                (OffsetDateTime::UNIX_EPOCH, None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(owner.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();
}

async fn seed_additional(database: &Path, ceremony: &CeremonyId) {
    let store = Arc::new(SqliteCeremonyStore::open(database).unwrap());
    let clock = Arc::new(SystemClock::new());
    StartBudgetedCeremonyUseCase::new(
        store.clone(),
        stream(&store),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
        BudgetLedgerService::new(
            Arc::new(SqliteBudgetLedgerStore::open(database).unwrap()),
            clock.clone(),
        ),
    )
    .execute(StartBudgetedCeremonyInput::new(
        StartCeremonyInput::new(
            ceremony.clone(),
            CeremonyName::new("http_worker_acceptance").unwrap(),
            CeremonyVersion::v1(),
            CeremonyContext::empty(),
            "acceptance-owner",
            AuditActorKind::Service,
        ),
        made_core::value_objects::BudgetLimits::new(
            BudgetQuantities::new(
                ExecutionDuration::from_micros(10_000),
                BudgetTokenCount::new(10),
                CostMicros::new(0),
                ToolCallCount::new(10),
            ),
            None,
        )
        .unwrap(),
    ))
    .await
    .unwrap();
    let owner = AuthenticatedPrincipal::new(
        PrincipalId::new("acceptance-owner").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    AuthorizationPolicyAdministrationService::new(
        AuthorizationPolicyId::new("worker-acceptance-policy").unwrap(),
        Arc::new(SqliteAuthorizationPolicyStore::open(database).unwrap()),
        clock,
    )
    .issue(
        &owner,
        AuthorizationGrant::new(
            AuthorizationGrantId::new(format!("worker-session-grant-{ceremony}")).unwrap(),
            PrincipalId::new("acceptance-worker").unwrap(),
            [
                AuthorizationAction::EnforceCeremonyDeadlines,
                AuthorizationAction::ClaimCeremonyStep,
            ],
            AuthorizationScope::Ceremony {
                ceremony_id: ceremony.clone(),
            },
            (OffsetDateTime::UNIX_EPOCH, None),
            DelegationDepth::none(),
            AuthorizationGrantIssuer::direct(owner.clone()),
        )
        .unwrap(),
    )
    .await
    .unwrap();
}

fn spawn_service(
    root: &Path,
    database: &Path,
    remote: &str,
    tls: &TlsMaterial,
    root_policies: Option<&str>,
) -> ServiceProcess {
    let grpc = TcpListener::bind("127.0.0.1:0").unwrap();
    let http = TcpListener::bind("127.0.0.1:0").unwrap();
    let grpc_port = grpc.local_addr().unwrap().port();
    let http_port = http.local_addr().unwrap().port();
    let ca = root.join("ca.pem");
    let cert = root.join("server.pem");
    let key = root.join("server-key.pem");
    let principals = root.join("principals.json");
    std::fs::write(&ca, &tls.ca_pem).unwrap();
    std::fs::write(&cert, &tls.server_cert_pem).unwrap();
    std::fs::write(&key, &tls.server_key_pem).unwrap();
    std::fs::write(
        &principals,
        serde_json::to_vec(&serde_json::json!([{
            "certificate_sha256": format!("{:x}", Sha256::digest(&tls.client_der)),
            "principal_id": "acceptance-owner",
            "principal_kind": "trusted_host"
        }]))
        .unwrap(),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_made"));
    for (name, _) in std::env::vars().filter(|(name, _)| name.starts_with("MADE_")) {
        command.env_remove(name);
    }
    command
        .env("MADE_NATS_ENABLED", "false")
        .env("MADE_MEMORY", "none")
        .env("MADE_GRPC_PORT", grpc_port.to_string())
        .env("MADE_HTTP_PORT", http_port.to_string())
        .env("MADE_CEREMONY_STORE_PATH", database)
        .env("MADE_GRPC_TLS_MODE", "mutual")
        .env("MADE_GRPC_TLS_CERT_PATH", cert)
        .env("MADE_GRPC_TLS_KEY_PATH", key)
        .env("MADE_GRPC_TLS_CLIENT_CA_PATH", ca)
        .env("MADE_AUTH_POLICY_ID", "worker-acceptance-policy")
        .env("MADE_AUTH_MTLS_PRINCIPALS_PATH", principals)
        .env("MADE_CEREMONY_STORE_ID", "worker-acceptance-store")
        .env("MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY", "a7".repeat(32))
        .env("MADE_WORKER_ENABLED", "true")
        .env("MADE_WORKER_CONNECTOR", "http")
        .env("MADE_WORKER_OWNER_ID", "acceptance-worker-owner")
        .env("MADE_WORKER_PRINCIPAL_ID", "acceptance-worker")
        .env("MADE_WORKER_OPERATION_ROOT", root.join("operations"))
        .env("MADE_WORKER_CAPACITY_DIRECTORY", root.join("capacity"))
        .env("MADE_WORKER_HTTP_BASE", remote)
        .env("MADE_WORKER_LEASE_TTL_MS", "3000")
        .env("MADE_WORKER_HEARTBEAT_MS", "500")
        .env("MADE_WORKER_CONNECTOR_TIMEOUT_MS", "2000")
        .env("MADE_WORKER_BUDGET_POLICY_VERSION", "1")
        .env("MADE_WORKER_BUDGET_MAX_DURATION_MICROS", "1000")
        .env("MADE_WORKER_BUDGET_MAX_TOKENS", "1")
        .env("MADE_WORKER_BUDGET_MAX_COST_MICROS", "0")
        .env("MADE_WORKER_BUDGET_MAX_TOOL_CALLS", "1")
        .env("RUST_LOG", "error")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    if let Some(root_policies) = root_policies {
        command
            .env("MADE_WORKER_ROOT_POLICIES_JSON", root_policies)
            .env("MADE_WORKER_MAX_PARALLEL", "1")
            .env("MADE_WORKER_SCHEDULER_CAPACITY", "1")
            .env("MADE_WORKER_CAPACITY_GLOBAL", "1");
    }
    drop(grpc);
    drop(http);
    ServiceProcess {
        child: command.spawn().unwrap(),
        grpc_port,
        http_port,
    }
}

async fn terminate(child: &mut ServiceProcess) {
    assert!(Command::new("kill")
        .args(["-TERM", &child.child.id().to_string()])
        .status()
        .unwrap()
        .success());
    let started = Instant::now();
    let deadline = started + shutdown_budget();
    loop {
        if let Some(status) = child.child.try_wait().unwrap() {
            assert!(
                status.success(),
                "service shutdown failed after {:?}: {status}",
                started.elapsed()
            );
            assert!(std::net::TcpStream::connect(("127.0.0.1", child.grpc_port)).is_err());
            assert!(std::net::TcpStream::connect(("127.0.0.1", child.http_port)).is_err());
            return;
        }
        assert!(
            Instant::now() < deadline,
            "service did not drain after SIGTERM within {:?} (elapsed {:?}); set MADE_TEST_TIMING_SCALE to widen test timing budgets under instrumentation",
            shutdown_budget(),
            started.elapsed()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Wall-clock multiplier for every fixed timing budget in this suite. Test
/// binaries running under llvm-cov instrumentation are several times slower
/// than a plain `cargo test` run; the coverage job sets this so the same
/// budgets hold instead of failing on runner speed. Defaults to 1.
fn timing_scale() -> f64 {
    std::env::var("MADE_TEST_TIMING_SCALE")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|scale| *scale >= 1.0)
        .unwrap_or(1.0)
}

/// A budget of `seconds` wall-clock seconds, scaled by the instrumentation
/// multiplier and left a generous margin above what a healthy drain needs.
fn shutdown_budget() -> Duration {
    Duration::from_secs_f64(10.0 * timing_scale())
}

/// Budget for the accept/recover loops that wait on the installed service.
/// The published receipts, remote effects and drains are waited on through
/// the same multiplier so an instrumented binary has proportionally longer.
fn acceptance_budget() -> Duration {
    Duration::from_secs_f64(30.0 * timing_scale())
}

#[tokio::test]
async fn composed_binary_executes_http_claim_persists_receipt_and_drains() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let database = root.join("made.sqlite3");
    let ceremony = CeremonyId::new("worker-service-acceptance").unwrap();
    seed(&root, &database, &ceremony).await;

    let state = AcceptanceState::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let remote = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/operations/:key", get(get_operation).put(put_operation))
        .with_state(state.clone());
    let remote_task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut service = spawn_service(&root, &database, &remote, &mint_tls(), None);
    let store = SqliteCeremonyStore::open(&database).unwrap();
    let started = Instant::now();
    let deadline = started + acceptance_budget();
    let receipt = loop {
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "made service exited early"
        );
        let operation_id = state
            .operations
            .lock()
            .unwrap()
            .values()
            .next()
            .and_then(|response| response["operation_id"].as_str().map(str::to_owned))
            .map(made_core::value_objects::ExecutionOperationId::new)
            .transpose()
            .unwrap();
        if let Some(operation_id) = operation_id {
            if let Some(receipt) = store.receipt(&operation_id).await.unwrap() {
                break receipt;
            }
        }
        assert!(
            Instant::now() < deadline,
            "worker did not persist an HTTP receipt; remote gets={}, puts={}, operations={:?}",
            *state.gets.lock().unwrap(),
            *state.puts.lock().unwrap(),
            *state.operations.lock().unwrap(),
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert_eq!(receipt.connector_id().as_str(), "http");
    assert_eq!(
        *state.puts.lock().unwrap(),
        1,
        "external effect executed once"
    );
    loop {
        let loaded = stream(&Arc::new(SqliteCeremonyStore::open(&database).unwrap()))
            .load(&ceremony)
            .await
            .unwrap();
        if loaded
            .instance
            .step_record(&made_core::value_objects::StepId::new("work").unwrap())
            .is_some_and(|record| record.status().is_success())
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "receipt was not applied to the ceremony"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    terminate(&mut service).await;
    remote_task.abort();
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Three installed daemons exercise durable recovery, not an in-process worker.
async fn competing_binaries_recover_a_killed_put_via_get_and_drain() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let database = root.join("made.sqlite3");
    let ceremony = CeremonyId::new("worker-competing-recovery").unwrap();
    seed(&root, &database, &ceremony).await;

    let state = AcceptanceState::default();
    // The connector records the operation before holding its response. Killing
    // the first binary at this barrier makes the durable remote GET the only
    // admissible recovery path for the other binaries.
    *state.response_delay.lock().unwrap() = Duration::from_secs(5);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let remote = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/operations/:key", get(get_operation).put(put_operation))
        .with_state(state.clone());
    let remote_task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let tls = mint_tls();
    let mut killed = spawn_service(&root, &database, &remote, &tls, None);
    let started = Instant::now();
    let deadline = started + acceptance_budget();
    while *state.puts.lock().unwrap() != 1 {
        assert!(
            killed.child.try_wait().unwrap().is_none(),
            "first competing daemon exited before its PUT barrier"
        );
        assert!(Instant::now() < deadline, "first daemon did not issue PUT");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let gets_before_crash = *state.gets.lock().unwrap();
    killed.child.kill().unwrap();
    assert!(!killed.child.wait().unwrap().success());

    // These are distinct installed binaries with their own listeners. They
    // share only the durable journal, capacity directory and remote connector.
    let mut recovering = spawn_service(&root, &database, &remote, &tls, None);
    let mut draining = spawn_service(&root, &database, &remote, &tls, None);
    let store = SqliteCeremonyStore::open(&database).unwrap();
    let operation_id = loop {
        let operation_id = state
            .operations
            .lock()
            .unwrap()
            .values()
            .next()
            .and_then(|response| response["operation_id"].as_str().map(str::to_owned))
            .map(made_core::value_objects::ExecutionOperationId::new)
            .transpose()
            .unwrap();
        if let Some(operation_id) = operation_id {
            if store.receipt(&operation_id).await.unwrap().is_some() {
                break operation_id;
            }
        }
        assert!(
            Instant::now() < deadline,
            "surviving daemons did not recover the killed operation; gets={}, puts={}",
            *state.gets.lock().unwrap(),
            *state.puts.lock().unwrap()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert_eq!(
        *state.puts.lock().unwrap(),
        1,
        "recovery replayed the effect"
    );
    assert!(
        *state.gets.lock().unwrap() > gets_before_crash,
        "recovery did not query the durable remote operation"
    );
    assert!(store.receipt(&operation_id).await.unwrap().is_some());

    // SIGTERM remains a drain protocol even while a peer stays alive. Both
    // listeners must close, proving the binary lifecycle rather than a task
    // cancellation inside this test process.
    terminate(&mut draining).await;
    terminate(&mut recovering).await;
    remote_task.abort();
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One installed daemon must reject every untrusted estimate before intent/PUT.
async fn installed_worker_budget_policy_fails_closed_before_intent_or_put() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let database = root.join("made.sqlite3");
    let ceremony = CeremonyId::new("worker-budget-fail-closed").unwrap();
    seed_with_definition(
        &root,
        &database,
        &ceremony,
        FAIL_CLOSED_BUDGET_DEFINITION,
        "http_worker_budget_fail_closed",
    )
    .await;
    let state = AcceptanceState::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let remote = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/operations/:key", get(get_operation).put(put_operation))
        .with_state(state.clone());
    let remote_task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut service = spawn_service(&root, &database, &remote, &mint_tls(), None);
    let store = Arc::new(SqliteCeremonyStore::open(&database).unwrap());
    let started = Instant::now();
    let deadline = started + acceptance_budget();
    let receipt = loop {
        let operation_id = state
            .operations
            .lock()
            .unwrap()
            .values()
            .next()
            .and_then(|response| response["operation_id"].as_str().map(str::to_owned))
            .map(made_core::value_objects::ExecutionOperationId::new)
            .transpose()
            .unwrap();
        if let Some(operation_id) = operation_id {
            if let Some(receipt) = store.receipt(&operation_id).await.unwrap() {
                break receipt;
            }
        }
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "installed worker exited before budget admission"
        );
        assert!(
            Instant::now() < deadline,
            "valid estimate did not reach receipt"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(
        *state.puts.lock().unwrap(),
        1,
        "only one estimate may reach PUT"
    );
    assert!(
        receipt.budget_measurement().is_unknown(),
        "the terminal connector response must remain Unknown, not be relabelled Estimated"
    );
    let loaded = loop {
        let loaded = stream(&store).load(&ceremony).await.unwrap();
        if loaded
            .instance
            .step_record(&made_core::value_objects::StepId::new("valid").unwrap())
            .is_some_and(|record| record.status().is_success())
        {
            break loaded;
        }
        assert!(Instant::now() < deadline, "valid receipt was not applied");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    for rejected in ["missing", "unknown", "stale", "over_ceiling"] {
        assert!(
            loaded
                .instance
                .step_record(&made_core::value_objects::StepId::new(rejected).unwrap())
                .is_some_and(|record| {
                    record.status() != made_core::value_objects::StepStatus::InProgress
                }),
            "{rejected} estimate was admitted into a step claim"
        );
    }
    // Keep the daemon alive across one extra scheduler period: malformed,
    // stale and over-ceiling metadata may retry admission but can never become
    // an intent or a second external operation.
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    assert_eq!(
        *state.puts.lock().unwrap(),
        1,
        "invalid estimate bypassed ceiling"
    );
    terminate(&mut service).await;
    remote_task.abort();
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // This is intentionally one causal lifecycle/recovery scenario.
async fn accepted_operation_keeps_draining_then_is_fenced_across_pause_cancel_and_restart() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let database = root.join("made.sqlite3");
    let ceremony = CeremonyId::new("worker-composed-lifecycle").unwrap();
    seed_with_definition(
        &root,
        &database,
        &ceremony,
        COMPOSED_LIFECYCLE_DEFINITION,
        "http_worker_composed_lifecycle",
    )
    .await;

    let state = AcceptanceState::default();
    let response_gate = Arc::new(tokio::sync::Notify::new());
    *state.response_gate.lock().unwrap() = Some(response_gate.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let remote = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/operations/:key", get(get_operation).put(put_operation))
        .with_state(state.clone());
    let remote_task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let root_policy = serde_json::json!({
        ceremony.as_str(): {
            "priority": 1, "weight": 1, "cost": 1, "requested_capacity": 1
        }
    })
    .to_string();
    let tls = mint_tls();
    let mut first = spawn_service(&root, &database, &remote, &tls, Some(&root_policy));
    let wall_deadline = Instant::now() + Duration::from_secs(15);
    while *state.puts.lock().unwrap() != 1 || state.operations.lock().unwrap().len() != 1 {
        assert!(
            first.child.try_wait().unwrap().is_none(),
            "installed worker exited before the accepted PUT"
        );
        assert!(
            Instant::now() < wall_deadline,
            "installed worker did not reach its accepted PUT"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let store = Arc::new(SqliteCeremonyStore::open(&database).unwrap());
    let session_stream = stream(&store);
    let repository = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    MountCeremonyDefinitionsUseCase::new(
        Arc::new(
            FileSystemCeremonyDefinitionSource::from_directory(root.join("definitions")).unwrap(),
        ),
        repository.clone(),
    )
    .execute()
    .await
    .unwrap();
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        repository,
        store.clone(),
    ));
    let clock = Arc::new(SystemClock::new());
    let accepted = session_stream.load(&ceremony).await.unwrap();
    let work = StepId::new("work").unwrap();
    let held_back = StepId::new("held_back").unwrap();
    let accepted_record = accepted
        .instance
        .step_record(&work)
        .expect("the first declared step must own the accepted operation");
    assert_eq!(accepted_record.status(), StepStatus::InProgress);
    let reservation_id = accepted_record
        .budget_reservation_id()
        .cloned()
        .expect("the accepted budgeted claim must retain its reservation");
    let accepted_fence = accepted.instance.step_claim_fence(&work).unwrap();
    let absolute_deadline = accepted.instance.step_deadlines()[&work].at();
    assert!(
        accepted
            .instance
            .step_record(&held_back)
            .is_none_or(|record| { record.status() != StepStatus::InProgress }),
        "scheduler admitted the second operation before pause"
    );

    // Tie the accepted remote effect to the exact durable semantic operation,
    // producer fence and budget reservation before lifecycle authority changes.
    // This prevents the lifecycle assertions below from accidentally observing
    // one claim while budget admission was proved by another fixture or step.
    let remote_response = state
        .operations
        .lock()
        .unwrap()
        .values()
        .next()
        .cloned()
        .expect("the accepted PUT must persist its remote response");
    let operation_id = made_core::value_objects::ExecutionOperationId::new(
        remote_response["operation_id"]
            .as_str()
            .expect("remote response must carry the operation id"),
    )
    .unwrap();
    let operation = store
        .operation(&operation_id)
        .await
        .unwrap()
        .expect("the accepted PUT must have a durable semantic operation");
    let intent = store
        .intent(&operation_id, &accepted_fence)
        .await
        .unwrap()
        .expect("the accepted producer fence must have a durable intent");
    assert_eq!(operation.ceremony_id(), &ceremony);
    assert_eq!(operation.step_id(), &work);
    assert_eq!(intent.operation(), &operation);
    assert_eq!(intent.claim_fence(), &accepted_fence);
    assert_eq!(
        remote_response["request_digest"].as_str(),
        Some(operation.request_digest().as_str())
    );
    assert_eq!(
        remote_response["producer_claim_fence"].as_str(),
        Some(accepted_fence.as_str())
    );
    let semantic_request: serde_json::Value =
        serde_json::from_slice(operation.request().as_bytes()).unwrap();
    assert_eq!(
        semantic_request["instance_id"].as_str(),
        Some(ceremony.as_str())
    );
    assert_eq!(semantic_request["step_id"].as_str(), Some(work.as_str()));
    assert_eq!(
        semantic_request["handler_config"]["budget_policy_version"].as_u64(),
        Some(1)
    );
    assert_eq!(
        semantic_request["handler_config"]["estimated_duration_micros"].as_u64(),
        Some(1_000)
    );
    assert_eq!(
        semantic_request["handler_config"]["estimated_tokens"].as_u64(),
        Some(1)
    );
    assert_eq!(
        semantic_request["handler_config"]["estimated_cost_micros"].as_u64(),
        Some(0)
    );
    assert_eq!(
        semantic_request["handler_config"]["estimated_tool_calls"].as_u64(),
        Some(1)
    );
    let account_id = accepted
        .instance
        .budget_account_id()
        .expect("budgeted ceremony must retain its ledger account");
    let ledger = SqliteBudgetLedgerStore::open(&database)
        .unwrap()
        .load(account_id)
        .await
        .unwrap()
        .expect("accepted claim must have a budget ledger");
    let reservation = ledger
        .ledger
        .reservations()
        .find(|reservation| reservation.id() == &reservation_id)
        .expect("accepted claim reservation must be durable");
    assert_eq!(reservation.operation_id().as_str(), operation_id.as_str());
    let expected_quantities = BudgetQuantities::new(
        ExecutionDuration::from_micros(1_000),
        BudgetTokenCount::new(1),
        CostMicros::new(0),
        ToolCallCount::new(1),
    );
    assert_eq!(reservation.quantities(), expected_quantities);
    assert_eq!(
        reservation.estimate().duration(),
        BudgetMeasurement::Estimated(expected_quantities.duration())
    );
    assert_eq!(
        reservation.estimate().tokens(),
        BudgetMeasurement::Estimated(expected_quantities.tokens())
    );
    assert_eq!(
        reservation.estimate().cost(),
        BudgetMeasurement::Estimated(expected_quantities.cost())
    );
    assert_eq!(
        reservation.estimate().tool_calls(),
        BudgetMeasurement::Estimated(expected_quantities.tool_calls())
    );
    assert!(
        reservation.reconciliation().is_none(),
        "the withheld remote response cannot reconcile admission before pause"
    );

    let paused = PauseCeremonyUseCase::new(resolver.clone(), session_stream.clone(), clock.clone())
        .execute(PauseCeremonyInput::new(
            ceremony.clone(),
            "acceptance-owner",
            AuditActorKind::Service,
            LifecycleReason::new("prove accepted work drains while admission closes").unwrap(),
        ))
        .await
        .unwrap();
    assert!(paused.is_paused());
    let paused_at = paused.updated_at();

    // The already accepted operation keeps its original identity and may
    // heartbeat while paused. Its renewal reaches, but never crosses, the
    // absolute deadline sealed by the original claim.
    let capped_renewal = loop {
        let records = session_stream.records(&ceremony).await.unwrap();
        if let Some(renewal) = records.iter().find_map(|record| match record.event() {
            Some(made_core::entities::CeremonyEvent::StepLeaseRenewed(renewed))
                if renewed.step_id == work
                    && renewed.claim_fence == accepted_fence
                    && renewed.renewed_at >= paused_at
                    && renewed.expires_at == absolute_deadline =>
            {
                Some(renewed.clone())
            }
            _ => None,
        }) {
            break renewal;
        }
        assert!(
            Instant::now() < wall_deadline,
            "paused accepted operation never renewed up to its absolute deadline"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(capped_renewal.expires_at, absolute_deadline);
    assert_eq!(
        *state.puts.lock().unwrap(),
        1,
        "pause admitted the held-back operation"
    );
    let still_paused = session_stream.load(&ceremony).await.unwrap();
    assert!(still_paused.instance.is_paused());
    assert!(
        still_paused
            .instance
            .step_record(&held_back)
            .is_none_or(|record| record.status() != StepStatus::InProgress),
        "pause did not close new admission"
    );

    CancelCeremonyUseCase::new(resolver.clone(), session_stream.clone(), clock)
        .execute(CancelCeremonyInput::new(
            ceremony.clone(),
            "acceptance-owner",
            AuditActorKind::Service,
            LifecycleReason::new("withdraw completion authority").unwrap(),
        ))
        .await
        .unwrap();

    // Let the next heartbeat observe cancellation and cancel the pending HTTP
    // response. The remote side has already persisted its response, but the
    // gate deliberately withholds it so losing authority wins deterministically.
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    assert!(store.receipt(&operation_id).await.unwrap().is_none());
    let cancelled = session_stream.load(&ceremony).await.unwrap();
    assert!(cancelled.instance.is_ended());
    assert_ne!(
        cancelled.instance.step_record(&work).unwrap().status(),
        StepStatus::Completed,
        "late remote result crossed cancelled authority"
    );
    assert_eq!(*state.puts.lock().unwrap(), 1);
    terminate(&mut first).await;

    let gets_before_restart = *state.gets.lock().unwrap();
    let mut recovering_a = spawn_service(&root, &database, &remote, &tls, Some(&root_policy));
    let mut recovering_b = spawn_service(&root, &database, &remote, &tls, Some(&root_policy));
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    assert_eq!(
        *state.puts.lock().unwrap(),
        1,
        "competing restart replayed the accepted external effect"
    );
    assert_eq!(
        *state.gets.lock().unwrap(),
        gets_before_restart,
        "competing restart queried a result after the claim fence lost authority"
    );
    assert!(store.receipt(&operation_id).await.unwrap().is_none());
    let after_restart = session_stream.load(&ceremony).await.unwrap();
    assert!(after_restart.instance.is_ended());
    assert_eq!(
        after_restart.instance.step_claim_fence(&work),
        Ok(accepted_fence),
        "restart replaced the original producer fence"
    );
    assert_ne!(
        after_restart.instance.step_record(&work).unwrap().status(),
        StepStatus::Completed,
        "restart attributed the late response to cancelled work"
    );
    terminate(&mut recovering_b).await;
    terminate(&mut recovering_a).await;
    response_gate.notify_waiters();
    remote_task.abort();
}

#[tokio::test]
async fn revoking_claim_authority_stops_heartbeat_renewal_and_drains() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let database = root.join("made.sqlite3");
    let ceremony = CeremonyId::new("worker-renewal-revocation").unwrap();
    seed(&root, &database, &ceremony).await;

    let state = AcceptanceState::default();
    *state.response_delay.lock().unwrap() = Duration::from_secs(3);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let remote = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/operations/:key", get(get_operation).put(put_operation))
        .with_state(state.clone());
    let remote_task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut service = spawn_service(&root, &database, &remote, &mint_tls(), None);
    let deadline = Instant::now() + Duration::from_secs(10);
    while *state.puts.lock().unwrap() == 0 {
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "made service exited early"
        );
        assert!(
            Instant::now() < deadline,
            "worker did not start the remote effect"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let owner = AuthenticatedPrincipal::new(
        PrincipalId::new("acceptance-owner").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    AuthorizationPolicyAdministrationService::new(
        AuthorizationPolicyId::new("worker-acceptance-policy").unwrap(),
        Arc::new(SqliteAuthorizationPolicyStore::open(&database).unwrap()),
        Arc::new(SystemClock::new()),
    )
    .revoke(
        &owner,
        &AuthorizationGrantId::new("worker-session-grant").unwrap(),
        AuthorizationRevocationReason::new("worker acceptance revocation").unwrap(),
    )
    .await
    .unwrap();
    let revoked_at = OffsetDateTime::now_utc();
    tokio::time::sleep(Duration::from_millis(1_300)).await;

    let records = stream(&Arc::new(SqliteCeremonyStore::open(&database).unwrap()))
        .records(&ceremony)
        .await
        .unwrap();
    let renewals_after_revocation = records
        .iter()
        .filter(|record| {
            matches!(
                record.event(),
                Some(made_core::entities::CeremonyEvent::StepLeaseRenewed(renewed))
                    if renewed.renewed_at > revoked_at
            )
        })
        .count();
    terminate(&mut service).await;
    remote_task.abort();
    assert_eq!(
        renewals_after_revocation, 0,
        "revoked claim kept renewing its lease"
    );
}

#[tokio::test]
async fn composed_daemon_exposes_distinct_root_policy_and_wait_reasons() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let database = root.join("made.sqlite3");
    let root_a = CeremonyId::new("worker-policy-root-a").unwrap();
    let root_b = CeremonyId::new("worker-policy-root-b").unwrap();
    seed(&root, &database, &root_a).await;
    seed_additional(&database, &root_b).await;

    let state = AcceptanceState::default();
    *state.response_delay.lock().unwrap() = Duration::from_millis(400);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let remote = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/operations/:key", get(get_operation).put(put_operation))
        .with_state(state);
    let remote_task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let policies = serde_json::json!({
        root_a.as_str(): {
            "priority": 1, "weight": 1, "cost": 1, "requested_capacity": 1
        },
        root_b.as_str(): {
            "priority": 9, "weight": 3, "cost": 1, "requested_capacity": 1
        }
    })
    .to_string();
    let mut service = spawn_service(&root, &database, &remote, &mint_tls(), Some(&policies));
    let started = Instant::now();
    let deadline = started + acceptance_budget();
    loop {
        let store = Arc::new(SqliteCeremonyStore::open(&database).unwrap());
        let completed = async {
            for ceremony in [&root_a, &root_b] {
                let loaded = stream(&store).load(ceremony).await.unwrap();
                if !loaded
                    .instance
                    .step_record(&made_core::value_objects::StepId::new("work").unwrap())
                    .is_some_and(|record| record.status().is_success())
                {
                    return false;
                }
            }
            true
        }
        .await;
        if completed {
            break;
        }
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "made service exited before both roots completed"
        );
        assert!(Instant::now() < deadline, "two-root worker run timed out");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    terminate(&mut service).await;
    remote_task.abort();

    let decisions = std::fs::read_to_string(root.join("capacity/admission-decisions.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert!(decisions.iter().any(|decision| {
        decision["root_id"] == root_a.as_str()
            && decision["weight"] == 1
            && decision["policy_version"] == 1
    }));
    assert!(decisions.iter().any(|decision| {
        decision["root_id"] == root_b.as_str()
            && decision["weight"] == 3
            && decision["priority"] == 9
    }));
    assert!(decisions.iter().any(|decision| {
        decision["reason"] == "backpressure" || decision["reason"] == "capacity"
    }));
    let first_admitted = decisions
        .iter()
        .find(|decision| decision["reason"] == "admitted")
        .unwrap();
    assert_eq!(
        first_admitted["root_id"],
        root_b.as_str(),
        "the higher-priority root must change the first operational decision"
    );
    assert!(
        decisions
            .iter()
            .filter(|decision| decision["reason"] == "admitted")
            .count()
            >= 2
    );
}
