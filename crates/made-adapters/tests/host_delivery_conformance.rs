//! The host-delivery core against its contracts, in memory.

use made_adapters::memory::{InMemoryHostDeliveryLedger, InMemoryIntegratorBindings};
use made_core::conformance::{HostDeliveryLedgerConformance, IntegratorBindingConformance};

#[tokio::test]
async fn the_in_memory_ledger_satisfies_the_contract() {
    let ledger = InMemoryHostDeliveryLedger::new();

    let passed = HostDeliveryLedgerConformance::run(&ledger)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 17, "properties run: {passed:?}");
    assert!(passed.contains(&"a_live_lease_excludes_another_host"));
    assert!(passed.contains(&"work_follows_a_replaced_role_when_asked_to"));
}

#[tokio::test]
async fn the_in_memory_bindings_satisfy_the_contract() {
    let bindings = InMemoryIntegratorBindings::new();

    let passed = IntegratorBindingConformance::run(&bindings)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 5, "properties run: {passed:?}");
    assert!(passed.contains(&"a_replacement_raises_the_fence"));
}
