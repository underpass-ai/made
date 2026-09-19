//! A reader keeps one budget snapshot while a valid writer commits between reads.
#![cfg(feature = "container-tests")]

use made_adapters::postgres::PostgresCeremonyStore;
use made_core::entities::{BudgetLedger, BudgetLedgerEvent};
use made_core::ports::{BudgetAppendOutcome, BudgetLedgerStorePort};
use made_core::value_objects::{
    BudgetAccountId, BudgetLedgerVersion, BudgetLimits, BudgetMeasurement, BudgetOperationId,
    BudgetQuantities, BudgetReservationEstimate, BudgetTokenCount, CostMicros, ExecutionDuration,
    ExecutionOperationId, ToolCallCount,
};
use made_tests_integration::postgres_fixture;
use sqlx::postgres::PgPoolOptions;
use time::OffsetDateTime;

#[tokio::test]
async fn a_concurrent_commit_cannot_mix_budget_head_and_event_versions() {
    let (pool, url, _container) = postgres_fixture::start_with_url().await;
    let store = PostgresCeremonyStore::new(pool);
    let account = BudgetAccountId::new("consistent-read").unwrap();
    let open = opened(account.clone());
    assert!(matches!(
        store
            .append(&account, BudgetLedgerVersion::EMPTY, vec![open.clone()])
            .await
            .unwrap(),
        BudgetAppendOutcome::Appended { .. }
    ));
    let reservation = BudgetLedger::rehydrate(&[open])
        .unwrap()
        .decide_reserve(
            BudgetOperationId::for_execution(&ExecutionOperationId::new("1".repeat(64)).unwrap()),
            BudgetReservationEstimate::new(
                BudgetMeasurement::Unknown,
                BudgetMeasurement::Estimated(BudgetTokenCount::new(1)),
                BudgetMeasurement::Unknown,
                BudgetMeasurement::Unknown,
            ),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
        .unwrap();
    let admin = PgPoolOptions::new().connect(&url).await.unwrap();
    let mut writer = admin.begin().await.unwrap();
    // Hold only the event table: the real adapter can read the version-one head
    // but cannot read events until the controlled writer commits version two.
    sqlx::query("LOCK TABLE ceremony_budget_ledger_events IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *writer)
        .await
        .unwrap();
    let reader_store = store.clone();
    let reader_account = account.clone();
    let reader = tokio::spawn(async move { reader_store.load(&reader_account).await });
    wait_for_event_read(&admin).await;
    sqlx::query("INSERT INTO ceremony_budget_ledger_events(account_id, version, payload) VALUES ($1, 2, $2)")
        .bind(account.as_str()).bind(serde_json::to_vec(&reservation).unwrap())
        .execute(&mut *writer).await.unwrap();
    sqlx::query("UPDATE ceremony_budget_ledgers SET version = 2 WHERE account_id = $1")
        .bind(account.as_str())
        .execute(&mut *writer)
        .await
        .unwrap();
    writer.commit().await.unwrap();
    let during = tokio::time::timeout(std::time::Duration::from_secs(5), reader)
        .await
        .unwrap()
        .unwrap()
        .expect("concurrent advance is not corrupt data")
        .unwrap();
    assert_eq!(during.version, BudgetLedgerVersion::new(1));
    assert_eq!(during.ledger.version(), during.version);
    let after = store.load(&account).await.unwrap().unwrap();
    assert_eq!(after.version, BudgetLedgerVersion::new(2));
    assert_eq!(after.ledger.version(), after.version);
}

fn opened(account_id: BudgetAccountId) -> BudgetLedgerEvent {
    BudgetLedgerEvent::Opened {
        account_id,
        limits: BudgetLimits::new(
            BudgetQuantities::new(
                ExecutionDuration::from_micros(0),
                BudgetTokenCount::new(100),
                CostMicros::new(0),
                ToolCallCount::new(0),
            ),
            None,
        )
        .unwrap(),
        opened_at: OffsetDateTime::UNIX_EPOCH,
    }
}

async fn wait_for_event_read(admin: &sqlx::PgPool) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type = 'Lock' \
                 AND query LIKE 'SELECT version, payload FROM ceremony_budget_ledger_events%')",
            )
            .fetch_one(admin)
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("reader reached its event query after reading the ledger head");
}
