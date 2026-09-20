//! The properties of offers that end without the host answering.
//!
//! Their own file because they are the one family where nothing the
//! host did decides anything: time ran out, the ceremony stopped being
//! worth asking about, or a full queue had to shed something. What
//! they all assert is the same refusal — the offer stays in the ledger
//! with its cause on it, because "nobody was ever asked this" and
//! "somebody was asked and said nothing" are the two answers an
//! operator most needs to tell apart.

use time::Duration;

use crate::ports::HostDeliveryLedgerPort;
use crate::value_objects::{
    DeliveryAttemptLimit, DeliveryExpiryCause, DurationMs, FollowReplacement, HostDeliveryMode,
    HostDeliveryObservationKind, HostDeliveryPolicy, HostDeliveryState, HostDeliveryStateKind,
    HostDeliveryTarget, ProcessedActionKind, ProcessedActionRef,
};

use super::host_delivery_fixtures::{call, ceremony, failure, observation, origin, record, role};
use super::host_delivery_ledger_steps::{enqueue, lease_all, lease_one, require_lease, state_of};
use super::ConformanceFailure;

/// The same lease length the rest of the suite offers.
const LEASE_MS: u64 = 30_000;

pub(super) async fn expiry_frees_a_lease_and_times_out_an_unclosed_hand_off(
    ledger: &dyn HostDeliveryLedgerPort,
) -> Result<(), ConformanceFailure> {
    const PROPERTY: &str = "expiry_frees_a_lease_and_times_out_an_unclosed_hand_off";
    let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
    let policy = HostDeliveryPolicy::new(
        HostDeliveryMode::PullLease,
        DurationMs::from_millis(LEASE_MS),
        Some(DurationMs::from_millis(1_000)),
        DeliveryAttemptLimit::default(),
        FollowReplacement::Stay,
    )
    .map_err(|error| failure(PROPERTY, error.to_string()))?;
    let offered = record(PROPERTY, "item", target.clone(), policy)?;
    let id = offered.id().clone();
    call(PROPERTY, ledger.enqueue(offered).await)?;

    let held = require_lease(PROPERTY, ledger, &target, "holder", origin()).await?;
    let seen = observation(PROPERTY, HostDeliveryObservationKind::Received, "taken")?;
    call(PROPERTY, ledger.acknowledge(&held, &seen, origin()).await)?;

    let later = origin() + Duration::milliseconds(2_000);
    let expired = call(PROPERTY, ledger.expire(later).await)?;
    if !expired.contains(&id) {
        return Err(failure(
            PROPERTY,
            "an acknowledgement nobody closed never timed out",
        ));
    }
    let stored = call(PROPERTY, ledger.get(&id).await)?
        .ok_or_else(|| failure(PROPERTY, "the expired delivery disappeared"))?;
    if stored.state().kind() != HostDeliveryStateKind::Expired {
        return Err(failure(PROPERTY, "expiry is not visible on the record"));
    }
    Ok(())
}

/// Nothing stays queued for a ceremony that is over, and nothing
/// that already ended is rewritten.
pub(super) async fn a_ceremony_that_ended_leaves_no_offer_waiting(
    ledger: &dyn HostDeliveryLedgerPort,
) -> Result<(), ConformanceFailure> {
    const PROPERTY: &str = "a_ceremony_that_ended_leaves_no_offer_waiting";
    let waiting = HostDeliveryTarget::role(role(PROPERTY, "waiting")?);
    let answered = HostDeliveryTarget::role(role(PROPERTY, "answered")?);
    enqueue(PROPERTY, ledger, "waiting-item", waiting.clone()).await?;
    enqueue(PROPERTY, ledger, "answered-item", answered.clone()).await?;

    // One of the two is closed before the ceremony ends, so the
    // property also says what must *not* move.
    let held = lease_one(PROPERTY, ledger, &answered, "host", origin())
        .await?
        .ok_or_else(|| failure(PROPERTY, "a queued delivery could not be leased"))?;
    let seen = observation(PROPERTY, HostDeliveryObservationKind::Received, "taken")?;
    call(PROPERTY, ledger.acknowledge(&held, &seen, origin()).await)?;
    call(
        PROPERTY,
        ledger
            .mark_processed(
                held.delivery_id(),
                held.owner(),
                None,
                &ProcessedActionRef::of(ProcessedActionKind::Responded),
                origin(),
            )
            .await,
    )?;

    let ended = origin() + Duration::seconds(1);
    let abandoned = call(
        PROPERTY,
        ledger
            .expire_ceremony(
                &ceremony(PROPERTY)?,
                DeliveryExpiryCause::CeremonyEnded,
                ended,
            )
            .await,
    )?;
    if abandoned.len() != 1 {
        return Err(failure(
            PROPERTY,
            "ending a ceremony did not abandon exactly the offer still open",
        ));
    }
    let still_waiting = state_of(PROPERTY, ledger, &waiting, "waiting-item").await?;
    if still_waiting != HostDeliveryStateKind::Expired {
        return Err(failure(
            PROPERTY,
            "an offer nobody took stayed open after its ceremony ended",
        ));
    }
    let closed = state_of(PROPERTY, ledger, &answered, "answered-item").await?;
    if closed != HostDeliveryStateKind::Processed {
        return Err(failure(
            PROPERTY,
            "a delivery that was already closed was overwritten by the ending",
        ));
    }
    Ok(())
}

/// An offer given up on stays in the ledger, with why.
///
/// The caller that has one item to give up on is backpressure: a
/// full queue sheds the oldest thing nobody is waiting on. Deleting
/// the row would make "the host was never told" look exactly like
/// "the host was told and did nothing", which is the first question
/// an operator asks. An ending already written is not overwritten.
pub(super) async fn an_offer_given_up_on_stays_readable_with_its_cause(
    ledger: &dyn HostDeliveryLedgerPort,
) -> Result<(), ConformanceFailure> {
    const PROPERTY: &str = "an_offer_given_up_on_stays_readable_with_its_cause";
    let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
    let id = enqueue(PROPERTY, ledger, "item", target.clone()).await?;

    let abandoned = call(
        PROPERTY,
        ledger
            .abandon(&id, DeliveryExpiryCause::QueueOverflow, origin())
            .await,
    )?
    .ok_or_else(|| failure(PROPERTY, "a queued offer could not be given up on"))?;
    let HostDeliveryState::Expired { cause, .. } = abandoned.state() else {
        return Err(failure(
            PROPERTY,
            "an abandoned offer is not visibly expired",
        ));
    };
    if *cause != DeliveryExpiryCause::QueueOverflow {
        return Err(failure(PROPERTY, "the reason for giving up was not kept"));
    }
    if !lease_all(PROPERTY, ledger, &target, "one", origin())
        .await?
        .is_empty()
    {
        return Err(failure(
            PROPERTY,
            "an abandoned offer was still handed to a host",
        ));
    }
    if call(
        PROPERTY,
        ledger
            .abandon(&id, DeliveryExpiryCause::CeremonyEnded, origin())
            .await,
    )?
    .is_some()
    {
        return Err(failure(
            PROPERTY,
            "an ending already written was given a second one",
        ));
    }
    Ok(())
}
