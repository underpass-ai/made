use time::{Duration, OffsetDateTime};

use crate::ports::{
    AckOutcome, DeliveryFailureOutcome, EnqueueOutcome, HostDeliveryFilter, HostDeliveryLedgerPort,
    HostDeliveryPageLimit, HostDeliveryTargetFilter, ProcessedOutcome,
};
use crate::value_objects::{
    DeliveryAttemptLimit, DeliveryExpiryCause, DeliveryFailureReason, DurationMs,
    FollowReplacement, HostDeliveryMode, HostDeliveryObservationKind, HostDeliveryPolicy,
    HostDeliveryStateKind, HostDeliveryTarget, IntegratorFence, ProcessedActionKind,
    ProcessedActionRef,
};

use super::host_delivery_fixtures::{
    call, ceremony, failure, forged_lease, incarnation, observation, origin, record, role,
};
use super::ConformanceFailure;

const LEASE_MS: u64 = 30_000;

/// Storage-independent properties of a durable host-delivery ledger.
#[derive(Debug)]
pub struct HostDeliveryLedgerConformance;

impl HostDeliveryLedgerConformance {
    pub async fn run(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::enqueue_is_idempotent_by_identity(ledger).await?;
        passed.push("enqueue_is_idempotent_by_identity");
        Self::a_live_lease_excludes_another_host(ledger).await?;
        passed.push("a_live_lease_excludes_another_host");
        Self::an_expired_lease_can_be_taken_over(ledger).await?;
        passed.push("an_expired_lease_can_be_taken_over");
        Self::acknowledging_requires_the_lease_that_holds_it(ledger).await?;
        passed.push("acknowledging_requires_the_lease_that_holds_it");
        Self::the_same_observation_twice_is_one_acknowledgement(ledger).await?;
        passed.push("the_same_observation_twice_is_one_acknowledgement");
        Self::a_different_observation_conflicts(ledger).await?;
        passed.push("a_different_observation_conflicts");
        Self::attempts_retry_until_the_limit_and_then_stay_failed(ledger).await?;
        passed.push("attempts_retry_until_the_limit_and_then_stay_failed");
        Self::processing_requires_an_acknowledgement_first(ledger).await?;
        passed.push("processing_requires_an_acknowledgement_first");
        Self::a_stale_fence_cannot_close_a_delivery(ledger).await?;
        passed.push("a_stale_fence_cannot_close_a_delivery");
        Self::an_exact_supersession_is_not_delivered_again(ledger).await?;
        passed.push("an_exact_supersession_is_not_delivered_again");
        Self::work_follows_a_replaced_role_when_asked_to(ledger).await?;
        passed.push("work_follows_a_replaced_role_when_asked_to");
        Self::expiry_frees_a_lease_and_times_out_an_unclosed_hand_off(ledger).await?;
        passed.push("expiry_frees_a_lease_and_times_out_an_unclosed_hand_off");
        Self::a_ceremony_that_ended_leaves_no_offer_waiting(ledger).await?;
        passed.push("a_ceremony_that_ended_leaves_no_offer_waiting");
        Ok(passed)
    }

    /// Nothing stays queued for a ceremony that is over, and nothing
    /// that already ended is rewritten.
    async fn a_ceremony_that_ended_leaves_no_offer_waiting(
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

    async fn enqueue_is_idempotent_by_identity(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "enqueue_is_idempotent_by_identity";
        let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
        let offered = record(PROPERTY, "item", target, HostDeliveryPolicy::default())?;

        let first = call(PROPERTY, ledger.enqueue(offered.clone()).await)?;
        if !matches!(first, EnqueueOutcome::Enqueued(_)) {
            return Err(failure(PROPERTY, "a new delivery was not accepted as new"));
        }
        let second = call(PROPERTY, ledger.enqueue(offered.clone()).await)?;
        let EnqueueOutcome::AlreadyQueued(existing) = second else {
            return Err(failure(
                PROPERTY,
                "the same item and destination produced a second delivery",
            ));
        };
        if existing.id() != offered.id() {
            return Err(failure(PROPERTY, "the existing delivery was not returned"));
        }
        Ok(())
    }

    async fn a_live_lease_excludes_another_host(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_live_lease_excludes_another_host";
        let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
        enqueue(PROPERTY, ledger, "item", target.clone()).await?;

        let first = lease_one(PROPERTY, ledger, &target, "first", origin()).await?;
        if first.is_none() {
            return Err(failure(PROPERTY, "a queued delivery could not be leased"));
        }
        let second = lease_all(PROPERTY, ledger, &target, "second", origin()).await?;
        if !second.is_empty() {
            return Err(failure(
                PROPERTY,
                "one delivery was handed to two hosts at once",
            ));
        }
        Ok(())
    }

    async fn an_expired_lease_can_be_taken_over(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_expired_lease_can_be_taken_over";
        let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
        enqueue(PROPERTY, ledger, "item", target.clone()).await?;
        lease_one(PROPERTY, ledger, &target, "first", origin()).await?;

        let later = origin() + Duration::milliseconds(LEASE_MS as i64 + 1);
        if lease_one(PROPERTY, ledger, &target, "second", later)
            .await?
            .is_none()
        {
            return Err(failure(
                PROPERTY,
                "a host that went away stranded the delivery it held",
            ));
        }
        Ok(())
    }

    async fn acknowledging_requires_the_lease_that_holds_it(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "acknowledging_requires_the_lease_that_holds_it";
        let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
        enqueue(PROPERTY, ledger, "item", target.clone()).await?;
        let held = require_lease(PROPERTY, ledger, &target, "holder", origin()).await?;
        let stranger = forged_lease(PROPERTY, &held)?;

        let seen = observation(PROPERTY, HostDeliveryObservationKind::Received, "taken")?;
        let refused = call(
            PROPERTY,
            ledger.acknowledge(&stranger, &seen, origin()).await,
        )?;
        if !matches!(refused, AckOutcome::LeaseNotOwned) {
            return Err(failure(
                PROPERTY,
                "a lease nobody issued was allowed to acknowledge",
            ));
        }
        let accepted = call(PROPERTY, ledger.acknowledge(&held, &seen, origin()).await)?;
        if !matches!(accepted, AckOutcome::Acknowledged(_)) {
            return Err(failure(PROPERTY, "the holder could not acknowledge"));
        }
        Ok(())
    }

    async fn the_same_observation_twice_is_one_acknowledgement(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "the_same_observation_twice_is_one_acknowledgement";
        let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
        enqueue(PROPERTY, ledger, "item", target.clone()).await?;
        let held = require_lease(PROPERTY, ledger, &target, "holder", origin()).await?;
        let seen = observation(PROPERTY, HostDeliveryObservationKind::Received, "taken")?;

        call(PROPERTY, ledger.acknowledge(&held, &seen, origin()).await)?;
        let repeated = call(PROPERTY, ledger.acknowledge(&held, &seen, origin()).await)?;
        if !matches!(repeated, AckOutcome::AlreadyAcknowledged(_)) {
            return Err(failure(
                PROPERTY,
                "a host that answered twice was treated as two answers",
            ));
        }
        Ok(())
    }

    async fn a_different_observation_conflicts(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_different_observation_conflicts";
        let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
        enqueue(PROPERTY, ledger, "item", target.clone()).await?;
        let held = require_lease(PROPERTY, ledger, &target, "holder", origin()).await?;

        let seen = observation(PROPERTY, HostDeliveryObservationKind::Received, "taken")?;
        call(PROPERTY, ledger.acknowledge(&held, &seen, origin()).await)?;
        let refused_it = observation(PROPERTY, HostDeliveryObservationKind::Refused, "no")?;
        let conflict = call(
            PROPERTY,
            ledger.acknowledge(&held, &refused_it, origin()).await,
        )?;
        let AckOutcome::Conflict { existing } = conflict else {
            return Err(failure(
                PROPERTY,
                "a second, different answer overwrote the first",
            ));
        };
        if existing.kind() != HostDeliveryObservationKind::Received {
            return Err(failure(
                PROPERTY,
                "the conflict did not carry what is stored",
            ));
        }
        Ok(())
    }

    async fn attempts_retry_until_the_limit_and_then_stay_failed(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "attempts_retry_until_the_limit_and_then_stay_failed";
        let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
        let policy = HostDeliveryPolicy::new(
            HostDeliveryMode::PullLease,
            DurationMs::from_millis(LEASE_MS),
            None,
            DeliveryAttemptLimit::new(2).map_err(|error| failure(PROPERTY, error.to_string()))?,
            FollowReplacement::Stay,
        )
        .map_err(|error| failure(PROPERTY, error.to_string()))?;
        let offered = record(PROPERTY, "item", target.clone(), policy)?;
        call(PROPERTY, ledger.enqueue(offered).await)?;

        let reason = DeliveryFailureReason::new("the host was busy")
            .map_err(|error| failure(PROPERTY, error.to_string()))?;
        let first = require_lease(PROPERTY, ledger, &target, "one", origin()).await?;
        let requeued = call(
            PROPERTY,
            ledger.mark_failed(&first, &reason, origin()).await,
        )?;
        if !matches!(requeued, DeliveryFailureOutcome::Requeued(_)) {
            return Err(failure(
                PROPERTY,
                "the first failure used up every attempt at once",
            ));
        }
        let second = require_lease(PROPERTY, ledger, &target, "two", origin()).await?;
        let exhausted = call(
            PROPERTY,
            ledger.mark_failed(&second, &reason, origin()).await,
        )?;
        let DeliveryFailureOutcome::Exhausted(record) = exhausted else {
            return Err(failure(PROPERTY, "the attempt limit was not reached"));
        };
        if record.state().kind() != HostDeliveryStateKind::Failed {
            return Err(failure(
                PROPERTY,
                "an exhausted delivery is not visibly failed",
            ));
        }
        let later = origin() + Duration::milliseconds(LEASE_MS as i64 + 1);
        if !lease_all(PROPERTY, ledger, &target, "three", later)
            .await?
            .is_empty()
        {
            return Err(failure(PROPERTY, "a failed delivery was offered again"));
        }
        Ok(())
    }

    async fn processing_requires_an_acknowledgement_first(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "processing_requires_an_acknowledgement_first";
        let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
        let queued = enqueue(PROPERTY, ledger, "item", target.clone()).await?;
        let owner = incarnation(PROPERTY, "holder")?;
        let action = ProcessedActionRef::of(ProcessedActionKind::Responded);

        let refused = call(
            PROPERTY,
            ledger
                .mark_processed(&queued, &owner, None, &action, origin())
                .await,
        )?;
        if !matches!(refused, ProcessedOutcome::NotAcknowledged { .. }) {
            return Err(failure(
                PROPERTY,
                "a delivery nobody received was closed as acted upon",
            ));
        }

        let held = require_lease(PROPERTY, ledger, &target, "holder", origin()).await?;
        let seen = observation(PROPERTY, HostDeliveryObservationKind::Received, "taken")?;
        call(PROPERTY, ledger.acknowledge(&held, &seen, origin()).await)?;
        let closed = call(
            PROPERTY,
            ledger
                .mark_processed(&queued, held.owner(), None, &action, origin())
                .await,
        )?;
        if !matches!(closed, ProcessedOutcome::Processed(_)) {
            return Err(failure(
                PROPERTY,
                "an acknowledged delivery could not be closed",
            ));
        }
        let repeated = call(
            PROPERTY,
            ledger
                .mark_processed(&queued, held.owner(), None, &action, origin())
                .await,
        )?;
        if !matches!(repeated, ProcessedOutcome::AlreadyProcessed(_)) {
            return Err(failure(PROPERTY, "closing twice was not a no-op"));
        }
        Ok(())
    }

    async fn a_stale_fence_cannot_close_a_delivery(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_stale_fence_cannot_close_a_delivery";
        let target = HostDeliveryTarget::role(role(PROPERTY, "seat")?);
        let queued = enqueue(PROPERTY, ledger, "item", target.clone()).await?;
        let held = require_lease(PROPERTY, ledger, &target, "holder", origin()).await?;
        let seen = observation(PROPERTY, HostDeliveryObservationKind::Received, "taken")?;
        call(PROPERTY, ledger.acknowledge(&held, &seen, origin()).await)?;

        let action = ProcessedActionRef::of(ProcessedActionKind::Integrated);
        let current = IntegratorFence::FIRST.next();
        call(
            PROPERTY,
            ledger
                .mark_processed(&queued, held.owner(), Some(current), &action, origin())
                .await,
        )?;
        let stale = call(
            PROPERTY,
            ledger
                .mark_processed(
                    &queued,
                    held.owner(),
                    Some(IntegratorFence::FIRST),
                    &ProcessedActionRef::of(ProcessedActionKind::NoAction),
                    origin(),
                )
                .await,
        )?;
        if !matches!(stale, ProcessedOutcome::FenceRejected { .. }) {
            return Err(failure(
                PROPERTY,
                "a host that was replaced was allowed to close the work",
            ));
        }
        Ok(())
    }

    async fn an_exact_supersession_is_not_delivered_again(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_exact_supersession_is_not_delivered_again";
        let previous = HostDeliveryTarget::role(role(PROPERTY, "before")?);
        let replacement = HostDeliveryTarget::role(role(PROPERTY, "after")?);
        let queued = enqueue(PROPERTY, ledger, "item", previous.clone()).await?;

        let outcome = call(
            PROPERTY,
            ledger
                .supersede(&previous, &replacement, FollowReplacement::Stay, origin())
                .await,
        )?;
        if outcome.superseded() != [queued.clone()] {
            return Err(failure(
                PROPERTY,
                "the pending delivery was not reported as superseded",
            ));
        }
        if !outcome.replacements().is_empty() {
            return Err(failure(
                PROPERTY,
                "work that was told to stay followed the replacement anyway",
            ));
        }
        if !lease_all(PROPERTY, ledger, &previous, "after", origin())
            .await?
            .is_empty()
        {
            return Err(failure(PROPERTY, "a superseded delivery was offered again"));
        }
        let stored = call(PROPERTY, ledger.get(&queued).await)?
            .ok_or_else(|| failure(PROPERTY, "the superseded delivery disappeared"))?;
        if stored.state().kind() != HostDeliveryStateKind::Superseded {
            return Err(failure(
                PROPERTY,
                "supersession is not visible on the record",
            ));
        }
        Ok(())
    }

    async fn work_follows_a_replaced_role_when_asked_to(
        ledger: &dyn HostDeliveryLedgerPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "work_follows_a_replaced_role_when_asked_to";
        let previous = HostDeliveryTarget::role(role(PROPERTY, "before")?);
        let replacement = HostDeliveryTarget::role(role(PROPERTY, "after")?);
        let queued = enqueue(PROPERTY, ledger, "item", previous.clone()).await?;

        let outcome = call(
            PROPERTY,
            ledger
                .supersede(&previous, &replacement, FollowReplacement::Follow, origin())
                .await,
        )?;
        let [opened] = outcome.replacements() else {
            return Err(failure(
                PROPERTY,
                "following a replacement opened no delivery for it",
            ));
        };
        if opened.target() != &replacement {
            return Err(failure(PROPERTY, "the replacement was addressed elsewhere"));
        }
        if opened.id() == &queued {
            return Err(failure(
                PROPERTY,
                "the replacement reused the superseded identity",
            ));
        }
        let stored = call(PROPERTY, ledger.get(&queued).await)?
            .ok_or_else(|| failure(PROPERTY, "the superseded delivery disappeared"))?;
        let Some(by) = stored.state().superseded_by() else {
            return Err(failure(
                PROPERTY,
                "the original does not point at what replaced it",
            ));
        };
        if by != opened.id() {
            return Err(failure(
                PROPERTY,
                "the superseded delivery does not name what replaced it",
            ));
        }
        Ok(())
    }

    async fn expiry_frees_a_lease_and_times_out_an_unclosed_hand_off(
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
}

async fn enqueue(
    property: &'static str,
    ledger: &dyn HostDeliveryLedgerPort,
    item_suffix: &str,
    target: HostDeliveryTarget,
) -> Result<crate::value_objects::HostDeliveryId, ConformanceFailure> {
    let offered = record(property, item_suffix, target, HostDeliveryPolicy::default())?;
    let id = offered.id().clone();
    call(property, ledger.enqueue(offered).await)?;
    Ok(id)
}

async fn lease_all(
    property: &'static str,
    ledger: &dyn HostDeliveryLedgerPort,
    target: &HostDeliveryTarget,
    owner_suffix: &str,
    now: OffsetDateTime,
) -> Result<Vec<crate::ports::LeasedDelivery>, ConformanceFailure> {
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

async fn lease_one(
    property: &'static str,
    ledger: &dyn HostDeliveryLedgerPort,
    target: &HostDeliveryTarget,
    owner_suffix: &str,
    now: OffsetDateTime,
) -> Result<Option<crate::value_objects::HostDeliveryLease>, ConformanceFailure> {
    Ok(lease_all(property, ledger, target, owner_suffix, now)
        .await?
        .into_iter()
        .next()
        .map(|leased| leased.into_parts().0))
}

/// Where one delivery has got to, looked up by what it is.
async fn state_of(
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

async fn require_lease(
    property: &'static str,
    ledger: &dyn HostDeliveryLedgerPort,
    target: &HostDeliveryTarget,
    owner_suffix: &str,
    now: OffsetDateTime,
) -> Result<crate::value_objects::HostDeliveryLease, ConformanceFailure> {
    lease_one(property, ledger, target, owner_suffix, now)
        .await?
        .ok_or_else(|| failure(property, "a queued delivery could not be leased"))
}
