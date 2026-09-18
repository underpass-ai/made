use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::entities::{BudgetLedger, BudgetLedgerEvent};
use crate::ports::{BudgetAppendOutcome, BudgetLedgerStorePort};
use crate::value_objects::{
    BudgetAccountId, BudgetLedgerVersion, BudgetLimits, BudgetMeasurement, BudgetOperationId,
    BudgetPageLimit, BudgetQuantities, BudgetReconciliationId, BudgetReservationEstimate,
    BudgetReservationId, BudgetTokenCount, CostMicros, CurrencyCode, ExecutionDuration,
    ExecutionOperationId, MeasuredBudgetQuantities, ToolCallCount,
};

use super::ConformanceFailure;

#[derive(Debug)]
pub struct BudgetLedgerStoreConformance;

impl BudgetLedgerStoreConformance {
    pub async fn run(
        store: &dyn BudgetLedgerStorePort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::append_reopens_the_same_ledger(store).await?;
        passed.push("append_reopens_the_same_ledger");
        Self::cas_admits_only_one_competing_reservation(store).await?;
        passed.push("cas_admits_only_one_competing_reservation");
        Self::pending_projection_tracks_reconciliation(store).await?;
        passed.push("pending_projection_tracks_reconciliation");
        Ok(passed)
    }

    async fn append_reopens_the_same_ledger(
        store: &dyn BudgetLedgerStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "append_reopens_the_same_ledger";
        let account = account(PROPERTY)?;
        append(
            store,
            PROPERTY,
            &account,
            BudgetLedgerVersion::EMPTY,
            vec![open_event(account.clone())],
        )
        .await?;
        let snapshot = call(PROPERTY, store.load(&account).await)?
            .ok_or_else(|| failure(PROPERTY, "opened ledger is absent"))?;
        if snapshot.version != BudgetLedgerVersion::new(1)
            || snapshot.ledger.account_id() != Some(&account)
        {
            return Err(failure(
                PROPERTY,
                "opened ledger did not round-trip at version one",
            ));
        }
        Ok(())
    }

    async fn cas_admits_only_one_competing_reservation(
        store: &dyn BudgetLedgerStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "cas_admits_only_one_competing_reservation";
        let account = account(PROPERTY)?;
        append(
            store,
            PROPERTY,
            &account,
            BudgetLedgerVersion::EMPTY,
            vec![open_event(account.clone())],
        )
        .await?;
        let ledger = call(PROPERTY, store.load(&account).await)?.unwrap().ledger;
        let left = ledger
            .decide_reserve(operation("left")?, estimate(60), OffsetDateTime::UNIX_EPOCH)
            .map_err(|error| failure(PROPERTY, error.to_string()))?
            .unwrap();
        let right = ledger
            .decide_reserve(
                operation("right")?,
                estimate(60),
                OffsetDateTime::UNIX_EPOCH,
            )
            .map_err(|error| failure(PROPERTY, error.to_string()))?
            .unwrap();
        append(
            store,
            PROPERTY,
            &account,
            BudgetLedgerVersion::new(1),
            vec![left],
        )
        .await?;
        let outcome = call(
            PROPERTY,
            store
                .append(&account, BudgetLedgerVersion::new(1), vec![right])
                .await,
        )?;
        if !matches!(outcome, BudgetAppendOutcome::Conflict { actual, .. } if actual == BudgetLedgerVersion::new(2))
        {
            return Err(failure(
                PROPERTY,
                "stale competing append was not a version-two conflict",
            ));
        }
        Ok(())
    }

    async fn pending_projection_tracks_reconciliation(
        store: &dyn BudgetLedgerStorePort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "pending_projection_tracks_reconciliation";
        let account = account(PROPERTY)?;
        append(
            store,
            PROPERTY,
            &account,
            BudgetLedgerVersion::EMPTY,
            vec![open_event(account.clone())],
        )
        .await?;
        let operation = operation("effect")?;
        let reservation_id = BudgetReservationId::for_operation(&account, &operation);
        let reserve = BudgetLedger::rehydrate(&[open_event(account.clone())])
            .map_err(|error| failure(PROPERTY, error.to_string()))?
            .decide_reserve(operation, estimate(20), OffsetDateTime::UNIX_EPOCH)
            .map_err(|error| failure(PROPERTY, error.to_string()))?
            .unwrap();
        append(
            store,
            PROPERTY,
            &account,
            BudgetLedgerVersion::new(1),
            vec![reserve],
        )
        .await?;
        let pending = call(
            PROPERTY,
            store
                .pending(None, BudgetPageLimit::new(500).unwrap())
                .await,
        )?;
        if !pending
            .reservations()
            .iter()
            .any(|item| item.id() == &reservation_id)
        {
            return Err(failure(
                PROPERTY,
                "live reservation is absent from pending projection",
            ));
        }
        let snapshot = call(PROPERTY, store.load(&account).await)?.unwrap();
        let measured = MeasuredBudgetQuantities::new(
            BudgetMeasurement::Unknown,
            BudgetMeasurement::Observed(BudgetTokenCount::new(18)),
            BudgetMeasurement::Unknown,
            BudgetMeasurement::Observed(ToolCallCount::new(1)),
        );
        let event = snapshot
            .ledger
            .decide_reconcile(
                reservation_id.clone(),
                BudgetReconciliationId::new(format!("receipt-{PROPERTY}"))
                    .map_err(|error| failure(PROPERTY, error.to_string()))?,
                measured,
                OffsetDateTime::UNIX_EPOCH,
            )
            .map_err(|error| failure(PROPERTY, error.to_string()))?
            .unwrap();
        append(store, PROPERTY, &account, snapshot.version, vec![event]).await?;
        let pending = call(
            PROPERTY,
            store
                .pending(None, BudgetPageLimit::new(500).unwrap())
                .await,
        )?;
        if pending
            .reservations()
            .iter()
            .any(|item| item.id() == &reservation_id)
        {
            return Err(failure(PROPERTY, "reconciled reservation remains pending"));
        }
        Ok(())
    }
}

async fn append(
    store: &dyn BudgetLedgerStorePort,
    property: &'static str,
    account: &BudgetAccountId,
    expected: BudgetLedgerVersion,
    events: Vec<BudgetLedgerEvent>,
) -> Result<(), ConformanceFailure> {
    let outcome = call(property, store.append(account, expected, events).await)?;
    if !matches!(outcome, BudgetAppendOutcome::Appended { .. }) {
        return Err(failure(property, "expected append conflicted"));
    }
    Ok(())
}
fn open_event(account_id: BudgetAccountId) -> BudgetLedgerEvent {
    BudgetLedgerEvent::Opened {
        account_id,
        limits: limits(),
        opened_at: OffsetDateTime::UNIX_EPOCH,
    }
}
fn limits() -> BudgetLimits {
    BudgetLimits::new(quantities(100), Some(CurrencyCode::new("EUR").unwrap())).unwrap()
}
fn quantities(tokens: u64) -> BudgetQuantities {
    BudgetQuantities::new(
        ExecutionDuration::from_micros(1_000),
        BudgetTokenCount::new(tokens),
        CostMicros::new(100),
        ToolCallCount::new(10),
    )
}
fn estimate(tokens: u64) -> BudgetReservationEstimate {
    BudgetReservationEstimate::new(
        BudgetMeasurement::Estimated(ExecutionDuration::from_micros(1_000)),
        BudgetMeasurement::Estimated(BudgetTokenCount::new(tokens)),
        BudgetMeasurement::Estimated(CostMicros::new(100)),
        BudgetMeasurement::Estimated(ToolCallCount::new(10)),
    )
}
fn account(property: &'static str) -> Result<BudgetAccountId, ConformanceFailure> {
    BudgetAccountId::new(format!("budget-conformance-{property}"))
        .map_err(|error| failure(property, error.to_string()))
}
fn operation(suffix: &str) -> Result<BudgetOperationId, ConformanceFailure> {
    let encoded = format!("{:x}", Sha256::digest(suffix.as_bytes()));
    ExecutionOperationId::new(encoded)
        .map(|operation| BudgetOperationId::for_execution(&operation))
        .map_err(|error| failure("fixture", error.to_string()))
}
fn call<T>(
    property: &'static str,
    result: Result<T, crate::DomainError>,
) -> Result<T, ConformanceFailure> {
    result.map_err(|error| failure(property, format!("adapter returned an error: {error}")))
}
fn failure(property: &'static str, detail: impl Into<String>) -> ConformanceFailure {
    ConformanceFailure::new(property, detail)
}
