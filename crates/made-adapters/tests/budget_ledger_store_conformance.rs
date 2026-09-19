use made_adapters::memory::InMemoryBudgetLedgerStore;
use made_core::conformance::BudgetLedgerStoreConformance;

#[tokio::test]
async fn in_memory_budget_ledger_satisfies_the_contract() {
    let store = InMemoryBudgetLedgerStore::new();
    let passed = BudgetLedgerStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(passed.len(), 3);
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sqlite_budget_ledger_satisfies_the_contract() {
    let directory = tempfile::tempdir().unwrap();
    let store = made_adapters::sqlite::SqliteBudgetLedgerStore::open(
        directory.path().join("budgets.sqlite3"),
    )
    .unwrap();
    let passed = BudgetLedgerStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(passed.len(), 3);
}
