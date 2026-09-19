use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use made_core::entities::{BudgetLedger, BudgetLedgerEvent};
use made_core::ports::{
    BudgetAppendOutcome, BudgetLedgerSnapshot, BudgetLedgerStorePort, BudgetReservationPage,
};
use made_core::value_objects::{
    BudgetAccountId, BudgetLedgerVersion, BudgetPageLimit, BudgetReservation, BudgetReservationId,
};
use made_core::DomainError;
use rusqlite::{params, Connection, TransactionBehavior};

#[derive(Debug, Clone)]
pub struct SqliteBudgetLedgerStore {
    path: PathBuf,
}

impl SqliteBudgetLedgerStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let store = Self {
            path: path.as_ref().to_path_buf(),
        };
        store.connection()?;
        Ok(store)
    }

    fn connection(&self) -> Result<Connection, DomainError> {
        let connection = Connection::open(&self.path).map_err(|error| sqlite_error(&error))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| sqlite_error(&error))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; CREATE TABLE IF NOT EXISTS budget_ledger_events (account_id TEXT NOT NULL, version INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(account_id, version)); CREATE TABLE IF NOT EXISTS budget_reservations (reservation_id TEXT PRIMARY KEY, account_id TEXT NOT NULL, pending INTEGER NOT NULL, payload BLOB NOT NULL); CREATE INDEX IF NOT EXISTS budget_reservations_pending ON budget_reservations(pending, reservation_id);").map_err(|error| sqlite_error(&error))?;
        Ok(connection)
    }

    async fn blocking<T, F>(&self, operation: &'static str, work: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(Self) -> Result<T, DomainError> + Send + 'static,
    {
        let store = self.clone();
        tokio::task::spawn_blocking(move || work(store))
            .await
            .map_err(|error| {
                tracing::error!(%error, operation, "budget SQLite blocking task failed");
                DomainError::InvariantViolated {
                    reason: "budget SQLite blocking task failed",
                }
            })?
    }
}

#[async_trait]
impl BudgetLedgerStorePort for SqliteBudgetLedgerStore {
    async fn load(
        &self,
        account: &BudgetAccountId,
    ) -> Result<Option<BudgetLedgerSnapshot>, DomainError> {
        let account = account.clone();
        self.blocking("load", move |store| {
            load_snapshot(&store.connection()?, &account)
        })
        .await
    }

    async fn append(
        &self,
        account: &BudgetAccountId,
        expected: BudgetLedgerVersion,
        events: Vec<BudgetLedgerEvent>,
    ) -> Result<BudgetAppendOutcome, DomainError> {
        if events.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "budget_events",
            });
        }
        if events.iter().any(|event| event.account_id() != account) {
            return Err(DomainError::InvariantViolated {
                reason: "budget append contains an event for another account",
            });
        }
        let account = account.clone();
        self.blocking("append", move |store| {
            append_events(store.connection()?, &account, expected, events)
        })
        .await
    }

    async fn pending(
        &self,
        after: Option<&BudgetReservationId>,
        limit: BudgetPageLimit,
    ) -> Result<BudgetReservationPage, DomainError> {
        let after = after
            .map(|value| value.as_str().to_owned())
            .unwrap_or_default();
        self.blocking("pending", move |store| {
            pending_reservations(&store.connection()?, &after, limit)
        })
        .await
    }
}

fn load_snapshot(
    connection: &Connection,
    account: &BudgetAccountId,
) -> Result<Option<BudgetLedgerSnapshot>, DomainError> {
    let events = load_events(connection, account)?;
    if events.is_empty() {
        return Ok(None);
    }
    let ledger = BudgetLedger::rehydrate(&events).map_err(|error| budget_decode_error(&error))?;
    Ok(Some(BudgetLedgerSnapshot {
        version: ledger.version(),
        ledger,
    }))
}

fn load_events(
    connection: &Connection,
    account: &BudgetAccountId,
) -> Result<Vec<BudgetLedgerEvent>, DomainError> {
    let mut statement = connection
        .prepare("SELECT payload FROM budget_ledger_events WHERE account_id = ?1 ORDER BY version")
        .map_err(|error| sqlite_error(&error))?;
    let rows = statement
        .query_map([account.as_str()], |row| row.get::<_, Vec<u8>>(0))
        .map_err(|error| sqlite_error(&error))?;
    rows.map(|row| decode(&row.map_err(|error| sqlite_error(&error))?))
        .collect()
}

fn append_events(
    mut connection: Connection,
    account: &BudgetAccountId,
    expected: BudgetLedgerVersion,
    events: Vec<BudgetLedgerEvent>,
) -> Result<BudgetAppendOutcome, DomainError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| sqlite_error(&error))?;
    let stored = load_events(&transaction, account)?;
    let actual = BudgetLedgerVersion::new(stored.len() as u64);
    if actual != expected {
        return Ok(BudgetAppendOutcome::Conflict { expected, actual });
    }
    let mut candidate = stored;
    candidate.extend(events.iter().cloned());
    let ledger =
        BudgetLedger::rehydrate(&candidate).map_err(|error| budget_decode_error(&error))?;
    let mut version = expected;
    for event in events {
        version = version.next();
        transaction.execute("INSERT INTO budget_ledger_events(account_id, version, payload) VALUES (?1, ?2, ?3)", params![account.as_str(), to_i64(version.value())?, encode(&event)?]).map_err(|error| sqlite_error(&error))?;
    }
    project_reservations(&transaction, account, &ledger)?;
    transaction.commit().map_err(|error| sqlite_error(&error))?;
    Ok(BudgetAppendOutcome::Appended { version })
}

fn project_reservations(
    connection: &Connection,
    account: &BudgetAccountId,
    ledger: &BudgetLedger,
) -> Result<(), DomainError> {
    for reservation in ledger.reservations() {
        connection.execute("INSERT INTO budget_reservations(reservation_id, account_id, pending, payload) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(reservation_id) DO UPDATE SET account_id=excluded.account_id, pending=excluded.pending, payload=excluded.payload", params![reservation.id().as_str(), account.as_str(), i64::from(reservation.reconciliation().is_none()), encode(reservation)?]).map_err(|error| sqlite_error(&error))?;
    }
    Ok(())
}

fn pending_reservations(
    connection: &Connection,
    after: &str,
    limit: BudgetPageLimit,
) -> Result<BudgetReservationPage, DomainError> {
    let mut statement = connection.prepare("SELECT payload FROM budget_reservations WHERE pending = 1 AND reservation_id > ?1 ORDER BY reservation_id LIMIT ?2").map_err(|error| sqlite_error(&error))?;
    let rows = statement
        .query_map(params![after, to_i64(limit.value() as u64)?], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .map_err(|error| sqlite_error(&error))?;
    let reservations = rows
        .map(|row| decode(&row.map_err(|error| sqlite_error(&error))?))
        .collect::<Result<Vec<BudgetReservation>, _>>()?;
    Ok(BudgetReservationPage::new(reservations))
}

fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, DomainError> {
    serde_json::to_vec(value).map_err(|error| encoding_error(&error))
}
fn decode<T: for<'de> serde::Deserialize<'de>>(bytes: &[u8]) -> Result<T, DomainError> {
    serde_json::from_slice(bytes).map_err(|error| encoding_error(&error))
}
fn to_i64(value: u64) -> Result<i64, DomainError> {
    i64::try_from(value).map_err(|_| DomainError::InvariantViolated {
        reason: "budget SQLite integer exceeds i64",
    })
}
fn sqlite_error(error: &rusqlite::Error) -> DomainError {
    tracing::error!(%error, "budget SQLite operation failed");
    DomainError::InvariantViolated {
        reason: "budget SQLite operation failed",
    }
}
fn encoding_error(error: &serde_json::Error) -> DomainError {
    tracing::error!(%error, "budget SQLite encoding failed");
    DomainError::InvariantViolated {
        reason: "budget SQLite encoding failed",
    }
}
fn budget_decode_error(error: &made_core::BudgetError) -> DomainError {
    tracing::error!(%error, "stored budget ledger violates its domain contract");
    DomainError::InvariantViolated {
        reason: "stored budget ledger violates its domain contract",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use made_app::budgets::{BudgetLedgerService, BudgetMutationOutcome};
    use made_core::value_objects::{
        ArtifactSourceKind, BudgetLimits, BudgetMeasurement, BudgetOperationId, BudgetQuantities,
        BudgetReservationEstimate, BudgetTokenCount, CeremonyId, CostMicros, CurrencyCode,
        ExecutionConnectorId, ExecutionDuration, ExecutionOperationId, ExecutionReceipt,
        ExecutionRecoveryCapability, ExecutionRequestDigest, MeasuredBudgetQuantities,
        StateIteration, StateVisit, StepClaimFence, StepId, StepIteration, StepOutput, StepResult,
        ToolCallCount,
    };
    use made_core::BudgetError;
    use time::OffsetDateTime;

    use crate::clock::SystemClock;

    use super::*;

    fn limits(tokens: u64) -> BudgetLimits {
        BudgetLimits::new(
            BudgetQuantities::new(
                ExecutionDuration::from_micros(1_000),
                BudgetTokenCount::new(tokens),
                CostMicros::new(1_000),
                ToolCallCount::new(100),
            ),
            Some(CurrencyCode::new("EUR").unwrap()),
        )
        .unwrap()
    }

    fn request(tokens: u64) -> BudgetReservationEstimate {
        BudgetReservationEstimate::new(
            BudgetMeasurement::Estimated(ExecutionDuration::from_micros(10)),
            BudgetMeasurement::Estimated(BudgetTokenCount::new(tokens)),
            BudgetMeasurement::Estimated(CostMicros::new(10)),
            BudgetMeasurement::Estimated(ToolCallCount::new(1)),
        )
    }

    fn execution_operation(label: &str) -> ExecutionOperationId {
        ExecutionOperationId::for_step(
            &CeremonyId::new(label).unwrap(),
            &StepId::new("work").unwrap(),
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
        )
    }

    fn operation(label: &str) -> BudgetOperationId {
        BudgetOperationId::for_execution(&execution_operation(label))
    }

    fn service(path: &Path) -> BudgetLedgerService {
        BudgetLedgerService::new(
            Arc::new(SqliteBudgetLedgerStore::open(path).unwrap()),
            Arc::new(SystemClock::new()),
        )
    }

    #[tokio::test]
    async fn two_hosts_cannot_reserve_past_one_root_balance_and_reopen_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("budget.sqlite3");
        let account = BudgetAccountId::new("shared-root").unwrap();
        let first = service(&path);
        first.open(account.clone(), limits(100)).await.unwrap();
        let second = service(&path);

        let left = first.reserve(&account, operation("parent-operation"), request(60));
        let right = second.reserve(&account, operation("child-operation"), request(60));
        let (left, right) = tokio::join!(left, right);
        let outcomes = [left, right];
        assert_eq!(outcomes.iter().filter(|item| item.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|item| matches!(item, Err(BudgetError::Exhausted { .. })))
                .count(),
            1
        );

        let reopened = service(&path).report(&account).await.unwrap();
        assert_eq!(reopened.reserved().tokens().value(), 60);
        assert_eq!(reopened.available().tokens().value(), 40);
    }

    #[tokio::test]
    async fn two_workers_share_one_idempotent_reservation_without_release_authority() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("budget.sqlite3");
        let account = BudgetAccountId::new("shared-root").unwrap();
        let first = service(&path);
        first.open(account.clone(), limits(100)).await.unwrap();
        let second = service(&path);
        let execution_operation = execution_operation("same-operation");
        let operation = BudgetOperationId::for_execution(&execution_operation);

        let (left, right) = tokio::join!(
            first.reserve(&account, operation.clone(), request(60)),
            second.reserve(&account, operation, request(60)),
        );
        let outcomes = [left.unwrap(), right.unwrap()];
        assert_eq!(
            outcomes
                .iter()
                .filter(|item| matches!(item, BudgetMutationOutcome::Applied { .. }))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|item| matches!(item, BudgetMutationOutcome::Existing { .. }))
                .count(),
            1
        );
        let pending = service(&path)
            .pending(None, BudgetPageLimit::new(10).unwrap())
            .await
            .unwrap();
        assert_eq!(pending.reservations().len(), 1);
        assert_eq!(
            service(&path)
                .report(&account)
                .await
                .unwrap()
                .reserved()
                .tokens()
                .value(),
            60
        );

        let measured = MeasuredBudgetQuantities::new(
            BudgetMeasurement::Unknown,
            BudgetMeasurement::Observed(BudgetTokenCount::new(120)),
            BudgetMeasurement::Estimated(CostMicros::new(8)),
            BudgetMeasurement::Observed(ToolCallCount::new(1)),
        );
        let receipt = ExecutionReceipt::new(
            execution_operation,
            ExecutionRequestDigest::new("1".repeat(64)).unwrap(),
            StepClaimFence::new("2".repeat(64)).unwrap(),
            ExecutionConnectorId::new("budget-test").unwrap(),
            None,
            ExecutionRecoveryCapability::IdempotentByOperationId,
            ArtifactSourceKind::NoOp,
            StepResult::completed(StepOutput::empty()).unwrap(),
            Vec::new(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
        .with_budget_measurement(measured);
        assert!(matches!(
            service(&path)
                .reconcile_receipt(&account, &receipt)
                .await
                .unwrap(),
            BudgetMutationOutcome::Applied { .. }
        ));
        assert!(matches!(
            service(&path)
                .reconcile_receipt(&account, &receipt)
                .await
                .unwrap(),
            BudgetMutationOutcome::Existing { .. }
        ));
        let report = service(&path).report(&account).await.unwrap();
        assert_eq!(report.observed().tokens().value(), 120);
        assert_eq!(report.overrun().tokens().value(), 20);
        assert_eq!(report.unconfirmed().duration().as_micros(), 10);
    }
}
