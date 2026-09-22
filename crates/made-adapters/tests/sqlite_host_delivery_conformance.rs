//! The durable host-delivery core against its contracts, and after a reopen.

#![cfg(feature = "sqlite")]

use made_adapters::sqlite::{
    SqliteCeremonyStore, SqliteHostDeliveryLedger, SqliteIntegratorBindings,
};
use made_core::conformance::{HostDeliveryLedgerConformance, IntegratorBindingConformance};
use made_core::ports::{
    BindReplacement, HostDeliveryLedgerPort, HostDeliveryQuery, IntegratorBindingPort,
};
use made_core::value_objects::{
    CeremonyId, CeremonyInterventionId, HostActivationMode, HostAddress, HostAgentIncarnation,
    HostDeliveryItem, HostDeliveryPolicy, HostDeliveryRecord, HostDeliveryStateKind,
    HostDestination, HostKind, IntegratorBinding, IntegratorBindingId, IntegratorScope, RoleId,
};
use std::path::Path;
use tempfile::TempDir;
use time::OffsetDateTime;

fn store(directory: &Path) -> SqliteCeremonyStore {
    SqliteCeremonyStore::open(directory.join("ceremonies.sqlite3")).expect("the store opens")
}

#[tokio::test]
async fn sqlite_satisfies_the_host_delivery_ledger_contract() {
    let directory = TempDir::new().expect("a temporary directory");
    let ledger = SqliteHostDeliveryLedger::open(directory.path().join("ceremonies.sqlite3"))
        .expect("the ledger opens");

    let passed = HostDeliveryLedgerConformance::run(&ledger)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 18, "properties run: {passed:?}");
}

#[tokio::test]
async fn sqlite_satisfies_the_integrator_binding_contract() {
    let directory = TempDir::new().expect("a temporary directory");
    let bindings = SqliteIntegratorBindings::open(directory.path().join("ceremonies.sqlite3"))
        .expect("the bindings open");

    let passed = IntegratorBindingConformance::run(&bindings)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 6, "properties run: {passed:?}");
}

/// A restart is the whole point of a durable ledger: what a host was
/// handed, what it said about it and who it was bound to all have to be
/// there when the process that wrote them is gone.
#[tokio::test]
async fn a_reopened_store_still_holds_every_delivery_and_binding() {
    let directory = TempDir::new().expect("a temporary directory");
    let ceremony = CeremonyId::new("c-reopen").unwrap();
    let role = RoleId::new("ENGINEER").unwrap();
    let record = HostDeliveryRecord::queued(
        HostDeliveryItem::intervention(
            ceremony.clone(),
            CeremonyInterventionId::new("i-1").unwrap(),
        ),
        made_core::value_objects::HostDeliveryTarget::role(role.clone()),
        HostDeliveryPolicy::default(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    let id = record.id().clone();
    let scope = IntegratorScope::ceremony(ceremony.clone());
    let binding = IntegratorBinding::new(
        IntegratorBindingId::new("b-1").unwrap(),
        scope.clone(),
        role,
        HostDestination::new(
            HostKind::new("generic").unwrap(),
            HostAddress::new("session-1").unwrap(),
            HostActivationMode::None,
        ),
        HostAgentIncarnation::new("run-1").unwrap(),
        OffsetDateTime::UNIX_EPOCH,
    );

    {
        let opened = store(directory.path());
        opened
            .host_delivery_ledger()
            .enqueue(record)
            .await
            .expect("the delivery is accepted");
        opened
            .integrator_bindings()
            .bind(binding.clone(), BindReplacement::Refuse)
            .await
            .expect("the binding is taken");
    }

    let reopened = store(directory.path());
    let stored = reopened
        .host_delivery_ledger()
        .get(&id)
        .await
        .expect("the ledger reads")
        .expect("the delivery survived the reopen");
    assert_eq!(stored.state().kind(), HostDeliveryStateKind::Queued);

    let listed = reopened
        .host_delivery_ledger()
        .list(&HostDeliveryQuery::new().in_ceremony(ceremony))
        .await
        .expect("the ledger lists");
    assert_eq!(listed.records().len(), 1);

    let current = reopened
        .integrator_bindings()
        .current(&scope)
        .await
        .expect("the bindings read")
        .expect("the binding survived the reopen");
    assert_eq!(current.id(), binding.id());
}
