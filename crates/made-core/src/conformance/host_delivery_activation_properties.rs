//! The properties of the half of the ledger the engine pushes.
//!
//! Their own file because they are a different conversation. Every
//! property next door is about a host that came and asked: it takes a
//! lease, it is excluded from, it answers or runs out of time. These
//! are about the engine going the other way — reaching out, holding
//! nothing, and writing down what the adapter said. What the two halves
//! must agree on is the one rule they keep restating: being reached is
//! not having answered.

use crate::ports::{HostActivationOutcome, HostDeliveryLedgerPort, RecordedActivation};
use crate::value_objects::{
    DeliveryAttemptLimit, DeliveryFailureReason, DurationMs, FollowReplacement,
    HostActivationAdapterKind, HostActivationReceipt, HostDeliveryMode, HostDeliveryPolicy,
    HostDeliveryStateKind, HostDeliveryTarget,
};

use super::host_delivery_fixtures::{call, failure, origin, record, role};
use super::host_delivery_ledger_steps::{enqueue, lease_all, state_of};
use super::ConformanceFailure;

/// The same lease length the rest of the suite offers.
const LEASE_MS: u64 = 30_000;

/// Reaching a host is transport, and transport is not an answer.
///
/// The receipt says the engine got through to somebody, which is a
/// claim by the adapter rather than by the host. A ledger that
/// treated it as the host having taken the work would strand the
/// item the first time a wake-up landed in a process that then
/// died, so a delivered record stays offerable.
pub(super) async fn a_host_that_was_reached_still_has_to_take_the_work(
    ledger: &dyn HostDeliveryLedgerPort,
) -> Result<(), ConformanceFailure> {
    const PROPERTY: &str = "a_host_that_was_reached_still_has_to_take_the_work";
    let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
    let id = enqueue(PROPERTY, ledger, "item", target.clone()).await?;

    let receipt = HostActivationReceipt::new(HostActivationAdapterKind::Command, origin(), None);
    let recorded = call(
        PROPERTY,
        ledger
            .record_activation(&id, &HostActivationOutcome::Accepted(receipt), origin())
            .await,
    )?;
    let RecordedActivation::Delivered(record) = recorded else {
        return Err(failure(PROPERTY, "a reached host was not written down"));
    };
    if record.state().kind() != HostDeliveryStateKind::DeliveredToHost {
        return Err(failure(
            PROPERTY,
            "the receipt did not move the delivery to delivered",
        ));
    }
    if lease_all(PROPERTY, ledger, &target, "one", origin())
        .await?
        .is_empty()
    {
        return Err(failure(
            PROPERTY,
            "a delivery pushed to a host stopped being offerable",
        ));
    }
    Ok(())
}

/// A deployment with no activation adapter is a normal deployment.
///
/// Its hosts ask for their own work, and nothing about the delivery
/// changes. Writing something anyway would make "nobody was woken"
/// indistinguishable from "somebody was woken and did nothing".
pub(super) async fn a_deployment_that_does_not_wake_hosts_writes_nothing(
    ledger: &dyn HostDeliveryLedgerPort,
) -> Result<(), ConformanceFailure> {
    const PROPERTY: &str = "a_deployment_that_does_not_wake_hosts_writes_nothing";
    let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
    let id = enqueue(PROPERTY, ledger, "item", target.clone()).await?;

    let recorded = call(
        PROPERTY,
        ledger
            .record_activation(&id, &HostActivationOutcome::Unsupported, origin())
            .await,
    )?;
    if !matches!(recorded, RecordedActivation::NotAttempted(_)) {
        return Err(failure(
            PROPERTY,
            "a deployment that wakes nobody reported something else",
        ));
    }
    if state_of(PROPERTY, ledger, &target, "item").await? != HostDeliveryStateKind::Queued {
        return Err(failure(
            PROPERTY,
            "an unsupported activation moved the delivery",
        ));
    }
    Ok(())
}

/// A host that cannot be reached is the same problem as one that
/// never answers, so a failed wake-up is counted like any other
/// attempt and gives up at the same limit.
pub(super) async fn a_wake_up_that_failed_costs_an_attempt(
    ledger: &dyn HostDeliveryLedgerPort,
) -> Result<(), ConformanceFailure> {
    const PROPERTY: &str = "a_wake_up_that_failed_costs_an_attempt";
    let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
    let policy = HostDeliveryPolicy::new(
        HostDeliveryMode::Activation,
        DurationMs::from_millis(LEASE_MS),
        None,
        DeliveryAttemptLimit::new(2).map_err(|error| failure(PROPERTY, error.to_string()))?,
        FollowReplacement::Stay,
    )
    .map_err(|error| failure(PROPERTY, error.to_string()))?;
    let offered = record(PROPERTY, "item", target.clone(), policy)?;
    let id = offered.id().clone();
    call(PROPERTY, ledger.enqueue(offered).await)?;

    let reason = DeliveryFailureReason::new("the host did not answer the door")
        .map_err(|error| failure(PROPERTY, error.to_string()))?;
    let first = call(
        PROPERTY,
        ledger
            .record_activation(
                &id,
                &HostActivationOutcome::Failed(reason.clone()),
                origin(),
            )
            .await,
    )?;
    if !matches!(first, RecordedActivation::Requeued(_)) {
        return Err(failure(
            PROPERTY,
            "the first failed wake-up used up every attempt at once",
        ));
    }
    let second = call(
        PROPERTY,
        ledger
            .record_activation(&id, &HostActivationOutcome::Failed(reason), origin())
            .await,
    )?;
    let RecordedActivation::Exhausted(record) = second else {
        return Err(failure(PROPERTY, "the attempt limit was not reached"));
    };
    if record.state().kind() != HostDeliveryStateKind::Failed {
        return Err(failure(
            PROPERTY,
            "a delivery nobody could be woken for is not visibly failed",
        ));
    }
    Ok(())
}

/// An activation reported against an identity the ledger does not
/// hold creates nothing. The adapter is outside the engine, and a
/// store that materialised a delivery from a receipt would let
/// whatever it talks to put rows in the ledger.
pub(super) async fn an_activation_of_an_unknown_delivery_invents_nothing(
    ledger: &dyn HostDeliveryLedgerPort,
) -> Result<(), ConformanceFailure> {
    const PROPERTY: &str = "an_activation_of_an_unknown_delivery_invents_nothing";
    let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
    let never_offered = record(PROPERTY, "item", target, HostDeliveryPolicy::default())?;

    let receipt = HostActivationReceipt::new(HostActivationAdapterKind::Command, origin(), None);
    let recorded = call(
        PROPERTY,
        ledger
            .record_activation(
                never_offered.id(),
                &HostActivationOutcome::Accepted(receipt),
                origin(),
            )
            .await,
    )?;
    if recorded != RecordedActivation::Unknown {
        return Err(failure(
            PROPERTY,
            "a receipt for nothing produced a delivery",
        ));
    }
    if call(PROPERTY, ledger.get(never_offered.id()).await)?.is_some() {
        return Err(failure(
            PROPERTY,
            "the ledger stored a delivery nobody offered",
        ));
    }
    Ok(())
}
