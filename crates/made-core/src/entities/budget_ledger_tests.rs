use time::OffsetDateTime;

use super::{BudgetLedger, BudgetLedgerEvent};
use crate::value_objects::{
    BudgetAccountId, BudgetDimension, BudgetLimits, BudgetMeasurement, BudgetOperationId,
    BudgetQuantities, BudgetReconciliationId, BudgetReservationId, BudgetTokenCount, CostMicros,
    ExecutionDuration, MeasuredBudgetQuantities, ToolCallCount,
};
use crate::BudgetError;

fn quantities(tokens: u64) -> BudgetQuantities {
    BudgetQuantities::new(
        ExecutionDuration::from_micros(100),
        BudgetTokenCount::new(tokens),
        CostMicros::new(10),
        ToolCallCount::new(1),
    )
}
fn limits() -> BudgetLimits {
    BudgetLimits::new(
        BudgetQuantities::new(
            ExecutionDuration::from_micros(1_000),
            BudgetTokenCount::new(100),
            CostMicros::new(100),
            ToolCallCount::new(10),
        ),
        Some(crate::value_objects::CurrencyCode::new("EUR").unwrap()),
    )
    .unwrap()
}
fn account() -> BudgetAccountId {
    BudgetAccountId::new("root").unwrap()
}
fn open() -> BudgetLedger {
    BudgetLedger::rehydrate(&[BudgetLedgerEvent::Opened {
        account_id: account(),
        limits: limits(),
        opened_at: OffsetDateTime::UNIX_EPOCH,
    }])
    .unwrap()
}

#[test]
fn two_operations_cannot_reserve_past_the_shared_limit() {
    let mut ledger = open();
    let event = ledger
        .decide_reserve(
            BudgetOperationId::new("parent").unwrap(),
            quantities(60),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
        .unwrap();
    ledger.apply(event).unwrap();
    let error = ledger
        .decide_reserve(
            BudgetOperationId::new("child").unwrap(),
            quantities(60),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        BudgetError::Exhausted {
            dimension: BudgetDimension::Tokens,
            ..
        }
    ));
}

#[test]
fn retry_returns_existing_without_an_event() {
    let mut ledger = open();
    let operation = BudgetOperationId::new("same-operation").unwrap();
    let event = ledger
        .decide_reserve(
            operation.clone(),
            quantities(25),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
        .unwrap();
    ledger.apply(event).unwrap();
    assert_eq!(
        ledger
            .decide_reserve(operation, quantities(25), OffsetDateTime::UNIX_EPOCH)
            .unwrap(),
        None
    );
    assert_eq!(ledger.balance().unwrap().reserved().tokens().value(), 25);
}

#[test]
fn unknown_stays_charged_and_observed_overrun_is_honest() {
    let mut ledger = open();
    let operation = BudgetOperationId::new("effect").unwrap();
    let reservation_id = BudgetReservationId::for_operation(&account(), &operation);
    ledger
        .apply(
            ledger
                .decide_reserve(operation, quantities(80), OffsetDateTime::UNIX_EPOCH)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    let measured = MeasuredBudgetQuantities::new(
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Observed(BudgetTokenCount::new(120)),
        BudgetMeasurement::Estimated(CostMicros::new(9)),
        BudgetMeasurement::Observed(ToolCallCount::new(1)),
    );
    ledger
        .apply(
            ledger
                .decide_reconcile(
                    reservation_id,
                    BudgetReconciliationId::new("receipt").unwrap(),
                    measured,
                    OffsetDateTime::UNIX_EPOCH,
                )
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    let balance = ledger.balance().unwrap();
    assert_eq!(balance.unconfirmed().duration().as_micros(), 100);
    assert_eq!(balance.observed().tokens().value(), 120);
    assert_eq!(balance.estimated().cost().value(), 9);
    assert_eq!(balance.overrun().tokens().value(), 20);
    assert_eq!(balance.available().tokens().value(), 0);
}

#[test]
fn a_reconciliation_replay_is_idempotent_but_a_change_conflicts() {
    let mut ledger = open();
    let operation = BudgetOperationId::new("effect").unwrap();
    let reservation_id = BudgetReservationId::for_operation(&account(), &operation);
    ledger
        .apply(
            ledger
                .decide_reserve(operation, quantities(20), OffsetDateTime::UNIX_EPOCH)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    let id = BudgetReconciliationId::new("receipt").unwrap();
    let measured = MeasuredBudgetQuantities::new(
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Observed(BudgetTokenCount::new(10)),
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Unknown,
    );
    ledger
        .apply(
            ledger
                .decide_reconcile(
                    reservation_id.clone(),
                    id.clone(),
                    measured,
                    OffsetDateTime::UNIX_EPOCH,
                )
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        ledger
            .decide_reconcile(
                reservation_id.clone(),
                id.clone(),
                measured,
                OffsetDateTime::UNIX_EPOCH
            )
            .unwrap(),
        None
    );
    let changed = MeasuredBudgetQuantities::new(
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Observed(BudgetTokenCount::new(11)),
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Unknown,
    );
    assert!(matches!(
        ledger.decide_reconcile(reservation_id, id, changed, OffsetDateTime::UNIX_EPOCH),
        Err(BudgetError::ReconciliationConflict(_))
    ));
}

#[test]
fn reconciliation_can_advance_knowledge_without_changing_its_receipt_identity() {
    let mut ledger = open();
    let operation = BudgetOperationId::new("later-observed").unwrap();
    let reservation_id = BudgetReservationId::for_operation(&account(), &operation);
    let event = ledger
        .decide_reserve(operation, quantities(20), OffsetDateTime::UNIX_EPOCH)
        .unwrap()
        .unwrap();
    ledger.apply(event).unwrap();
    let id = BudgetReconciliationId::new("receipt-later").unwrap();
    let unknown = MeasuredBudgetQuantities::new(
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Unknown,
    );
    let event = ledger
        .decide_reconcile(
            reservation_id.clone(),
            id.clone(),
            unknown,
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
        .unwrap();
    ledger.apply(event).unwrap();
    let observed = MeasuredBudgetQuantities::new(
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Observed(BudgetTokenCount::new(18)),
        BudgetMeasurement::Unknown,
        BudgetMeasurement::Unknown,
    );
    let event = ledger
        .decide_reconcile(reservation_id, id, observed, OffsetDateTime::UNIX_EPOCH)
        .unwrap()
        .unwrap();
    ledger.apply(event).unwrap();
    assert_eq!(ledger.balance().unwrap().observed().tokens().value(), 18);
    assert_eq!(ledger.balance().unwrap().unconfirmed().tokens().value(), 0);
}
