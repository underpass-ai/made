//! The delivery core, through the facade an operator actually opens.
//!
//! Composing it is not the same as composing it *durably*: a default
//! that quietly stayed in memory would pass every adapter contract and
//! still lose every question a supervisor asked, the first time the
//! process restarted.

use made_core::ports::{BindReplacement, HostDeliveryQuery};
use made_core::value_objects::{
    CeremonyId, CeremonyInterventionId, HostActivationAdapterKind, HostActivationMode, HostAddress,
    HostAgentIncarnation, HostDeliveryItem, HostDeliveryPolicy, HostDeliveryRecord,
    HostDeliveryStateKind, HostDeliveryTarget, HostDestination, HostKind, IntegratorBinding,
    IntegratorBindingId, IntegratorScope, RoleId,
};
use made_embedded::EmbeddedMade;
use time::OffsetDateTime;

fn delivery(ceremony: &CeremonyId) -> HostDeliveryRecord {
    HostDeliveryRecord::queued(
        HostDeliveryItem::intervention(
            ceremony.clone(),
            CeremonyInterventionId::new("i-1").expect("a valid intervention id"),
        ),
        HostDeliveryTarget::role(RoleId::new("ENGINEER").expect("a valid role")),
        HostDeliveryPolicy::default(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .expect("a valid delivery")
}

fn binding(ceremony: &CeremonyId) -> IntegratorBinding {
    IntegratorBinding::new(
        IntegratorBindingId::new("b-1").expect("a valid binding id"),
        IntegratorScope::ceremony(ceremony.clone()),
        RoleId::new("INTEGRATOR").expect("a valid role"),
        HostDestination::new(
            HostKind::new("generic").expect("a valid host kind"),
            HostAddress::new("session-1").expect("a valid address"),
            HostActivationMode::None,
        ),
        HostAgentIncarnation::new("run-1").expect("a valid incarnation"),
        OffsetDateTime::UNIX_EPOCH,
    )
}

#[tokio::test]
async fn deliveries_and_bindings_survive_reopening_the_durable_engine() {
    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("made.sqlite3");
    let ceremony = CeremonyId::new("c-durable").expect("a valid ceremony id");
    let record = delivery(&ceremony);
    let id = record.id().clone();
    let scope = IntegratorScope::ceremony(ceremony.clone());

    {
        let engine = EmbeddedMade::open(&path).expect("the durable engine opens");
        engine
            .host_delivery_ledger()
            .enqueue(record)
            .await
            .expect("the delivery is accepted");
        engine
            .integrator_bindings()
            .bind(binding(&ceremony), BindReplacement::Refuse)
            .await
            .expect("the binding is taken");
    }

    let reopened = EmbeddedMade::open(&path).expect("the durable engine reopens");
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
    assert_eq!(current.id().as_str(), "b-1");
}

/// The default deployment does not wake hosts, and says so rather than
/// reporting a hand-off it never made.
#[tokio::test]
async fn the_default_engine_declares_that_it_activates_nobody() {
    let engine = EmbeddedMade::builder().build();

    assert_eq!(
        engine.host_activation().kind(),
        HostActivationAdapterKind::None
    );
}
