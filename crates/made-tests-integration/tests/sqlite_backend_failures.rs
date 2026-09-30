//! A SQLite backend failure is the server's fault, on both wires (#268).
//!
//! The SQLite authorization and budget stores used to answer a driver
//! failure in a way a client read as its own mistake (`invalid_argument`
//! over gRPC, `invalid_request` over MCP) and, in the budget store, logged
//! the driver's rendered text. Each test here makes SQLite itself refuse a
//! write with a trigger whose message stands for anything the driver could
//! echo (a stored value, a path), and checks three things: the domain
//! variant, the code both wires answer, and a structured log that names
//! the phase and the SQLite codes but none of the values.
//!
//! The stores run their work on blocking threads, so the log is captured
//! by a process-wide subscriber: this binary holds only these tests.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use made_adapters::clock::SystemClock;
use made_adapters::grpc::{budget_error_to_status, domain_error_to_status};
use made_adapters::sqlite::{SqliteAuthorizationPolicyStore, SqliteBudgetLedgerStore};
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
};
use made_app::budgets::BudgetLedgerService;
use made_core::ports::{AuthorizationPolicyStorePort, ClockPort};
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionTtl,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationRequest, AuthorizationRequestId, AuthorizationScope, AuthorizationTargetDigest,
    BudgetAccountId, BudgetLimits, BudgetMeasurement, BudgetOperationId, BudgetQuantities,
    BudgetReservationEstimate, BudgetTokenCount, CeremonyId, CostMicros, CurrencyCode,
    DelegationDepth, ExecutionDuration, ExecutionOperationId, PrincipalId, PrincipalKind,
    StateIteration, StateVisit, StepId, StepIteration, ToolCallCount,
};
use made_core::{BudgetError, DomainError};
use made_mcp::protocol::{ToolError, ToolErrorCode};
use time::{macros::datetime, Duration, OffsetDateTime};

const NOW: OffsetDateTime = datetime!(2026-09-19 12:00:00 UTC);
/// What the trigger makes SQLite say: stands for any text the driver echoes.
const DRIVER_MARKER: &str = "driver-echo-7f3a";
/// `SQLITE_CONSTRAINT_TRIGGER`, what `RAISE(ABORT, ...)` reports.
const CONSTRAINT_TRIGGER: i32 = 1811;

#[tokio::test]
async fn authorization_store_failure_is_failed_precondition_and_logs_no_value() {
    let log = captured_log();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authorization.db");
    let store = Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap());
    let clock = Arc::new(FixedClock(NOW));
    let policy_id = AuthorizationPolicyId::new("sqlite-backend-failure").unwrap();
    let admin = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    admin.open(trusted_host(), Vec::new()).await.unwrap();
    admin.issue(&trusted_host(), worker_grant()).await.unwrap();
    reject_inserts_into(&path, "authorization_decisions");
    let authorize = AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock,
        AuthorizationDecisionTtl::from_seconds(30).unwrap(),
    );

    let error = authorize
        .execute(AuthorizationRequest::new(
            AuthorizationRequestId::new("secret-request-id").unwrap(),
            worker(),
            AuthorizationAction::ClaimCeremonyStep,
            AuthorizationScope::Global,
            AuthorizationTargetDigest::for_bytes(b"secret-target"),
        ))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        DomainError::InvariantViolated {
            reason: "sqlite: authorization persistence backend failed"
        }
    );
    assert_eq!(
        domain_error_to_status(error.clone()).code(),
        tonic::Code::FailedPrecondition
    );
    assert_eq!(ToolError::from(error).code(), ToolErrorCode::Refused);
    let logged = log.contents();
    assert!(
        logged.contains(&format!(
            "project authorization decision failed: sqlite error ConstraintViolation \
             (extended code {CONSTRAINT_TRIGGER})"
        )),
        "{logged}"
    );
    for value in [
        DRIVER_MARKER,
        "secret-request-id",
        "secret-target",
        path_text(&path),
    ] {
        assert!(!logged.contains(value), "{value} leaked into: {logged}");
    }
    assert!(
        store
            .decision_for_request(
                &policy_id,
                &AuthorizationRequestId::new("secret-request-id").unwrap()
            )
            .await
            .unwrap()
            .is_none(),
        "the failed append must roll back"
    );
}

#[tokio::test]
async fn budget_store_failure_is_failed_precondition_and_logs_no_value() {
    let log = captured_log();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("budget.db");
    let service = BudgetLedgerService::new(
        Arc::new(SqliteBudgetLedgerStore::open(&path).unwrap()),
        Arc::new(SystemClock::new()),
    );
    let account = BudgetAccountId::new("secret-account").unwrap();
    service.open(account.clone(), limits()).await.unwrap();
    reject_inserts_into(&path, "budget_ledger_events");

    let error = service
        .reserve(&account, operation(), estimate())
        .await
        .unwrap_err();

    let BudgetError::Persistence(domain) = &error else {
        panic!("expected a persistence failure, got {error:?}");
    };
    assert_eq!(
        domain,
        &DomainError::InvariantViolated {
            reason: "sqlite: budget persistence backend failed"
        }
    );
    assert_eq!(
        budget_error_to_status(error.clone()).code(),
        tonic::Code::FailedPrecondition
    );
    assert_eq!(ToolError::from(error).code(), ToolErrorCode::Refused);
    let logged = log.contents();
    assert!(
        logged.contains(&format!(
            "append budget event failed: sqlite error ConstraintViolation \
             (extended code {CONSTRAINT_TRIGGER})"
        )),
        "{logged}"
    );
    for value in [DRIVER_MARKER, "secret-account", path_text(&path)] {
        assert!(!logged.contains(value), "{value} leaked into: {logged}");
    }
}

/// A second connection, as another host would hold, makes SQLite refuse
/// every later insert into `table` with a message only the driver knows.
fn reject_inserts_into(path: &Path, table: &str) {
    rusqlite::Connection::open(path)
        .unwrap()
        .execute_batch(&format!(
            "CREATE TRIGGER reject_{table} BEFORE INSERT ON {table} \
             BEGIN SELECT RAISE(ABORT, '{DRIVER_MARKER}'); END;"
        ))
        .unwrap();
}

fn path_text(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn trusted_host() -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new("trusted-host").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap()
}

fn worker() -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new("worker").unwrap(),
        PrincipalKind::Worker,
        AuthenticationMethod::MutualTls,
    )
    .unwrap()
}

fn worker_grant() -> AuthorizationGrant {
    AuthorizationGrant::new(
        AuthorizationGrantId::new("worker-grant").unwrap(),
        worker().id().clone(),
        [AuthorizationAction::ClaimCeremonyStep],
        AuthorizationScope::Global,
        (NOW - Duration::seconds(1), Some(NOW + Duration::minutes(5))),
        DelegationDepth::none(),
        AuthorizationGrantIssuer::direct(trusted_host()),
    )
    .unwrap()
}

fn limits() -> BudgetLimits {
    BudgetLimits::new(
        BudgetQuantities::new(
            ExecutionDuration::from_micros(1_000),
            BudgetTokenCount::new(100),
            CostMicros::new(1_000),
            ToolCallCount::new(100),
        ),
        Some(CurrencyCode::new("EUR").unwrap()),
    )
    .unwrap()
}

fn estimate() -> BudgetReservationEstimate {
    BudgetReservationEstimate::new(
        BudgetMeasurement::Estimated(ExecutionDuration::from_micros(10)),
        BudgetMeasurement::Estimated(BudgetTokenCount::new(10)),
        BudgetMeasurement::Estimated(CostMicros::new(10)),
        BudgetMeasurement::Estimated(ToolCallCount::new(1)),
    )
}

fn operation() -> BudgetOperationId {
    BudgetOperationId::for_execution(&ExecutionOperationId::for_step(
        &CeremonyId::new("budget-failure").unwrap(),
        &StepId::new("work").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
    ))
}

#[derive(Debug, Clone, Copy)]
struct FixedClock(OffsetDateTime);

impl ClockPort for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

/// The process-wide log every test in this binary writes to. Each test
/// looks only for its own phase and its own values in it.
fn captured_log() -> CapturedLog {
    static LOG: OnceLock<CapturedLog> = OnceLock::new();
    LOG.get_or_init(|| {
        let log = CapturedLog::default();
        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_writer(log.clone())
                .with_ansi(false)
                .finish(),
        )
        .expect("this binary installs the only global subscriber");
        log
    })
    .clone()
}

/// A `MakeWriter` that keeps everything the subscriber writes.
#[derive(Clone, Default)]
struct CapturedLog(Arc<Mutex<Vec<u8>>>);

impl CapturedLog {
    fn contents(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl std::io::Write for CapturedLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for CapturedLog {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}
