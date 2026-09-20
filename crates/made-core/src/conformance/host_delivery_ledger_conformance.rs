use time::Duration;

use crate::ports::{
    AckOutcome, DeliveryFailureOutcome, EnqueueOutcome, HostDeliveryLedgerPort, ProcessedOutcome,
};
use crate::value_objects::{
    DeliveryAttemptLimit, DeliveryFailureReason, DurationMs, FollowReplacement, HostDeliveryMode,
    HostDeliveryObservationKind, HostDeliveryPolicy, HostDeliveryStateKind, HostDeliveryTarget,
    IntegratorFence, ProcessedActionKind, ProcessedActionRef,
};

use super::host_delivery_activation_properties as activation;
use super::host_delivery_ending_properties as ending;
use super::host_delivery_fixtures::{
    call, failure, forged_lease, incarnation, observation, origin, record, role,
};
use super::host_delivery_ledger_steps::{enqueue, lease_all, lease_one, require_lease};
use super::ConformanceFailure;

/// The same lease length the steps hand out, named here because the
/// properties reason about what happens after it runs out.
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
        ending::expiry_frees_a_lease_and_times_out_an_unclosed_hand_off(ledger).await?;
        passed.push("expiry_frees_a_lease_and_times_out_an_unclosed_hand_off");
        ending::a_ceremony_that_ended_leaves_no_offer_waiting(ledger).await?;
        passed.push("a_ceremony_that_ended_leaves_no_offer_waiting");
        ending::an_offer_given_up_on_stays_readable_with_its_cause(ledger).await?;
        passed.push("an_offer_given_up_on_stays_readable_with_its_cause");
        activation::a_host_that_was_reached_still_has_to_take_the_work(ledger).await?;
        passed.push("a_host_that_was_reached_still_has_to_take_the_work");
        activation::a_deployment_that_does_not_wake_hosts_writes_nothing(ledger).await?;
        passed.push("a_deployment_that_does_not_wake_hosts_writes_nothing");
        activation::a_wake_up_that_failed_costs_an_attempt(ledger).await?;
        passed.push("a_wake_up_that_failed_costs_an_attempt");
        activation::an_activation_of_an_unknown_delivery_invents_nothing(ledger).await?;
        passed.push("an_activation_of_an_unknown_delivery_invents_nothing");
        Ok(passed)
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
}
