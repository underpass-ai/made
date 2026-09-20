//! The durable host-delivery core against its contracts, on PostgreSQL.
//!
//! The third adapter of the ledger, and the one nothing was holding to
//! the contract: memory and SQLite both run the suite, and a deployment
//! that keeps its streams in PostgreSQL was taking the same promises on
//! trust. The properties are the storage-independent ones, so a
//! difference here is a difference in the adapter and nowhere else.

#![cfg(feature = "container-tests")]

use made_adapters::postgres::{PostgresHostDeliveryLedger, PostgresIntegratorBindings};
use made_core::conformance::{HostDeliveryLedgerConformance, IntegratorBindingConformance};
use made_core::ports::{HostDeliveryLedgerPort, HostDeliveryQuery};
use made_core::value_objects::{
    CeremonyId, CeremonyInterventionId, DeliveryExpiryCause, HostDeliveryItem, HostDeliveryPolicy,
    HostDeliveryRecord, HostDeliveryStateKind, HostDeliveryTarget, RoleId,
};
use made_tests_integration::postgres_fixture;
use time::{Duration, OffsetDateTime};

#[tokio::test]
async fn postgres_satisfies_the_host_delivery_ledger_contract() {
    let (pool, _container) = postgres_fixture::start().await;
    let ledger = PostgresHostDeliveryLedger::new(pool);

    let passed = HostDeliveryLedgerConformance::run(&ledger)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 18, "properties run: {passed:?}");
}

#[tokio::test]
async fn postgres_satisfies_the_integrator_binding_contract() {
    let (pool, _container) = postgres_fixture::start().await;
    let bindings = PostgresIntegratorBindings::new(pool);

    let passed = IntegratorBindingConformance::run(&bindings)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 5, "properties run: {passed:?}");
}

/// Ending one ceremony leaves the others alone.
///
/// The conformance suite proves an ended ceremony strands nothing; what
/// it cannot prove, because it runs one ceremony at a time, is that the
/// scan is bounded by the ceremony it was given. On a shared database
/// that is the difference between closing one session's questions and
/// closing everybody's.
#[tokio::test]
async fn ending_one_ceremony_leaves_another_ceremonys_offers_alone() {
    let (pool, _container) = postgres_fixture::start().await;
    let ledger = PostgresHostDeliveryLedger::new(pool);
    let at = OffsetDateTime::UNIX_EPOCH;

    for ceremony in ["ending", "carrying-on"] {
        let record = HostDeliveryRecord::queued(
            HostDeliveryItem::intervention(
                CeremonyId::new(ceremony).unwrap(),
                CeremonyInterventionId::new("item-1").unwrap(),
            ),
            HostDeliveryTarget::role(RoleId::new("ENGINEER").unwrap()),
            HostDeliveryPolicy::default(),
            at,
        )
        .unwrap();
        ledger.enqueue(record).await.expect("the offer is accepted");
    }

    let abandoned = ledger
        .expire_ceremony(
            &CeremonyId::new("ending").unwrap(),
            DeliveryExpiryCause::CeremonyEnded,
            at + Duration::seconds(1),
        )
        .await
        .expect("the ledger gives up on what that ceremony left");
    assert_eq!(abandoned.len(), 1, "abandoned: {abandoned:?}");

    let survivor = ledger
        .list(&HostDeliveryQuery::new().in_ceremony(CeremonyId::new("carrying-on").unwrap()))
        .await
        .expect("the ledger lists");
    assert_eq!(survivor.records().len(), 1);
    assert_eq!(
        survivor.records()[0].state().kind(),
        HostDeliveryStateKind::Queued,
        "ending one ceremony closed another's offer"
    );
}
