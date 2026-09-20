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
    MountCeremonyDefinitionsUseCase, PublishCeremonyDefinitionUseCase, StartCeremonyInput,
};
use made_core::ports::{
    CeremonyDefinitionRepositoryPort, ExecutionReceiptStorePort, NoopCeremonyEventSubscriber,
};
use made_core::value_objects::{
    AuditActorKind, AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationRevocationReason, AuthorizationScope, BudgetQuantities, BudgetTokenCount,
    CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion, CostMicros, DelegationDepth,
    ExecutionDuration, PrincipalId, PrincipalKind, StepOutput, StepResult, ToolCallCount,
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

#[derive(Clone, Default)]
struct AcceptanceState {
    operations: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    gets: Arc<Mutex<usize>>,
    puts: Arc<Mutex<usize>>,
    response_delay: Arc<Mutex<Duration>>,
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
    let delay = *state.response_delay.lock().unwrap();
    tokio::time::sleep(delay).await;
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
    let definitions_path = root.join("definitions");
    std::fs::create_dir_all(&definitions_path).unwrap();
    std::fs::write(definitions_path.join("worker.yaml"), DEFINITION).unwrap();
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
            &CeremonyName::new("http_worker_acceptance").unwrap(),
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
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.child.try_wait().unwrap() {
            assert!(status.success(), "service shutdown failed: {status}");
            assert!(std::net::TcpStream::connect(("127.0.0.1", child.grpc_port)).is_err());
            assert!(std::net::TcpStream::connect(("127.0.0.1", child.http_port)).is_err());
            return;
        }
        assert!(
            Instant::now() < deadline,
            "service did not drain after SIGTERM"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
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
    let deadline = Instant::now() + Duration::from_secs(15);
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
    let deadline = Instant::now() + Duration::from_secs(15);
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
    let deadline = Instant::now() + Duration::from_secs(15);
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
