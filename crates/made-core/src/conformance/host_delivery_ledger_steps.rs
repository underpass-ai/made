//! The steps every ledger property is written in terms of.
//!
//! Their own file because the suite next door is the contract and these
//! are only how it asks: one place to offer a delivery, one to take it,
//! one to look at what became of it. A property that built its own
//! would be testing its own construction as much as the adapter's.

use time::OffsetDateTime;

use crate::ports::{
    HostDeliveryFilter, HostDeliveryLedgerPort, HostDeliveryPageLimit, HostDeliveryTargetFilter,
    LeasedDelivery,
};
use crate::value_objects::{
    DurationMs, HostDeliveryId, HostDeliveryLease, HostDeliveryPolicy, HostDeliveryStateKind,
    HostDeliveryTarget,
};

use super::host_delivery_fixtures::{call, ceremony, failure, incarnation, record};
use super::ConformanceFailure;

/// The same lease length every property in the suite offers.
const LEASE_MS: u64 = 30_000;

pub(super) async fn enqueue(
    property: &'static str,
    ledger: &dyn HostDeliveryLedgerPort,
    item_suffix: &str,
    target: HostDeliveryTarget,
) -> Result<HostDeliveryId, ConformanceFailure> {
    let offered = record(property, item_suffix, target, HostDeliveryPolicy::default())?;
    let id = offered.id().clone();
    call(property, ledger.enqueue(offered).await)?;
    Ok(id)
}

pub(super) async fn lease_all(
    property: &'static str,
    ledger: &dyn HostDeliveryLedgerPort,
    target: &HostDeliveryTarget,
    owner_suffix: &str,
    now: OffsetDateTime,
) -> Result<Vec<LeasedDelivery>, ConformanceFailure> {
    let filter = HostDeliveryFilter::to(
        HostDeliveryTargetFilter::any_of([target.clone()])
            .map_err(|error| failure(property, error.to_string()))?,
    )
    .in_ceremony(ceremony(property)?);
    call(
        property,
        ledger
            .lease(
                &filter,
                &incarnation(property, owner_suffix)?,
                now,
                DurationMs::from_millis(LEASE_MS),
                HostDeliveryPageLimit::default(),
            )
            .await,
    )
}

pub(super) async fn lease_one(
    property: &'static str,
    ledger: &dyn HostDeliveryLedgerPort,
    target: &HostDeliveryTarget,
    owner_suffix: &str,
    now: OffsetDateTime,
) -> Result<Option<HostDeliveryLease>, ConformanceFailure> {
    Ok(lease_all(property, ledger, target, owner_suffix, now)
        .await?
        .into_iter()
        .next()
        .map(|leased| leased.into_parts().0))
}

/// Where one delivery has got to, looked up by what it is.
pub(super) async fn state_of(
    property: &'static str,
    ledger: &dyn HostDeliveryLedgerPort,
    target: &HostDeliveryTarget,
    item_suffix: &str,
) -> Result<HostDeliveryStateKind, ConformanceFailure> {
    let offered = record(
        property,
        item_suffix,
        target.clone(),
        HostDeliveryPolicy::default(),
    )?;
    let found = call(property, ledger.get(offered.id()).await)?
        .ok_or_else(|| failure(property, "a delivery that was enqueued cannot be read back"))?;
    Ok(found.state().kind())
}

pub(super) async fn require_lease(
    property: &'static str,
    ledger: &dyn HostDeliveryLedgerPort,
    target: &HostDeliveryTarget,
    owner_suffix: &str,
    now: OffsetDateTime,
) -> Result<HostDeliveryLease, ConformanceFailure> {
    lease_one(property, ledger, target, owner_suffix, now)
        .await?
        .ok_or_else(|| failure(property, "a queued delivery could not be leased"))
}
