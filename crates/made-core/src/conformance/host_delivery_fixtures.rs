//! Shared construction for the host-delivery contract suites.
//!
//! Its own file so both suites build the same shapes: two suites that
//! each invented their own record would agree with each other and with
//! no adapter in particular.

use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{
    DeliveryNote, HostAgentIncarnation, HostDeliveryItem, HostDeliveryLease, HostDeliveryLeaseId,
    HostDeliveryObservation, HostDeliveryObservationKind, HostDeliveryPolicy, HostDeliveryRecord,
    HostDeliveryTarget, ProtocolId, ProtocolInterventionId, RoleId,
};

use super::ConformanceFailure;

/// The instant every suite starts from, so nothing depends on the clock.
pub(super) fn origin() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

pub(super) fn failure(property: &'static str, detail: impl Into<String>) -> ConformanceFailure {
    ConformanceFailure::new(property, detail)
}

pub(super) fn call<T>(
    property: &'static str,
    outcome: Result<T, DomainError>,
) -> Result<T, ConformanceFailure> {
    outcome.map_err(|error| failure(property, format!("the adapter returned an error: {error}")))
}

/// A protocol of this property's own, so suites cannot collide in a
/// store they all share.
pub(super) fn protocol(property: &'static str) -> Result<ProtocolId, ConformanceFailure> {
    ProtocolId::new(property.replace('_', "-"))
        .map_err(|error| failure(property, error.to_string()))
}

pub(super) fn incarnation(
    property: &'static str,
    suffix: &str,
) -> Result<HostAgentIncarnation, ConformanceFailure> {
    HostAgentIncarnation::new(format!("{property}-{suffix}"))
        .map_err(|error| failure(property, error.to_string()))
}

pub(super) fn role(property: &'static str, suffix: &str) -> Result<RoleId, ConformanceFailure> {
    RoleId::new(format!("{property}-{suffix}"))
        .map_err(|error| failure(property, error.to_string()))
}

/// One queued intervention delivery for `target`.
pub(super) fn record(
    property: &'static str,
    item_suffix: &str,
    target: HostDeliveryTarget,
    policy: HostDeliveryPolicy,
) -> Result<HostDeliveryRecord, ConformanceFailure> {
    let intervention = ProtocolInterventionId::new(format!("{property}-{item_suffix}"))
        .map_err(|error| failure(property, error.to_string()))?;
    HostDeliveryRecord::queued(
        HostDeliveryItem::intervention(protocol(property)?, intervention),
        target,
        policy,
        origin(),
    )
    .map_err(|error| failure(property, error.to_string()))
}

pub(super) fn observation(
    property: &'static str,
    kind: HostDeliveryObservationKind,
    note: &str,
) -> Result<HostDeliveryObservation, ConformanceFailure> {
    Ok(HostDeliveryObservation::new(
        kind,
        origin(),
        None,
        DeliveryNote::new(note).map_err(|error| failure(property, error.to_string()))?,
    ))
}

/// A lease nobody issued, for proving that a stranger cannot commit.
pub(super) fn forged_lease(
    property: &'static str,
    held: &HostDeliveryLease,
) -> Result<HostDeliveryLease, ConformanceFailure> {
    Ok(HostDeliveryLease::new(
        held.delivery_id().clone(),
        HostDeliveryLeaseId::new(format!("{property}-forged"))
            .map_err(|error| failure(property, error.to_string()))?,
        held.owner().clone(),
        held.leased_until(),
    ))
}
