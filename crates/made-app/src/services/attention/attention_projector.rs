//! [`AttentionProjector`] — one bounded walk of the global feed, for
//! one bound integrator.
//!
//! # Why a cursor and not a subscriber
//!
//! Being told when something is appended is how the rest of the engine
//! reacts, and it is not enough here. A wake-up can be missed — the
//! process dies between the append and the reaction — and the loop
//! this feeds is supposed to survive a restart without a human
//! noticing. A durable cursor per binding means the feed itself is the
//! memory: reopen the store, walk on from where the cursor stopped,
//! and the work that was owed is owed again.
//!
//! # Why the cursor advances past records that produce nothing
//!
//! Most of the global feed belongs to somebody else. A cursor that
//! only moved when it found something would stop at the first record
//! of another ceremony and never reach its own.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{
    CeremonyEventCursorPort, CeremonyEventStorePort, ClockPort, EnqueueOutcome,
    HostActivationOutcome, HostActivationPort, HostDeliveryLedgerPort, PositionedRecord,
};
use made_core::value_objects::{
    CeremonyEventCursorLeaseId, CeremonyEventPageLimit, CeremonyEventQuarantineReason, DurationMs,
    HostActivationEnvelope, HostActivationMode, HostDeliveryItem, HostDeliveryPolicy,
    HostDeliveryRecord,
};

use super::{
    attention_for, queue_overflow, AttentionAudience, AttentionBackpressure, AttentionEvent,
    ProjectionRound,
};

/// As long as the publisher's, because the work is the same shape: one
/// position at a time, with an expiry so a worker that dies frees it.
const LEASE_DURATION: DurationMs = DurationMs::from_millis(30_000);

/// After this many failures at one position, the position is set aside
/// rather than blocking every later one behind it.
const MAX_ATTEMPTS: u32 = 3;

/// Derives attention from the feed and hands it to the delivery ledger.
pub struct AttentionProjector {
    events: Arc<dyn CeremonyEventStorePort>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
    activation: Arc<dyn HostActivationPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for AttentionProjector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AttentionProjector")
            .finish_non_exhaustive()
    }
}

impl AttentionProjector {
    #[must_use]
    pub fn new(
        events: Arc<dyn CeremonyEventStorePort>,
        cursors: Arc<dyn CeremonyEventCursorPort>,
        deliveries: Arc<dyn HostDeliveryLedgerPort>,
        activation: Arc<dyn HostActivationPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            events,
            cursors,
            deliveries,
            activation,
            clock,
        }
    }

    /// Walk at most `limit` positions of the feed for this audience.
    ///
    /// Bounded on purpose: the caller owns the loop and the decision to
    /// go round again, so a projector that ran until the feed was
    /// exhausted would take a thread hostage on a busy deployment.
    pub async fn project(
        &self,
        audience: &AttentionAudience,
        limit: CeremonyEventPageLimit,
    ) -> Result<ProjectionRound, DomainError> {
        let consumer = audience.consumer()?;
        let one = CeremonyEventPageLimit::new(1)?;
        let mut round = ProjectionRound::default();
        for _ in 0..limit.value() {
            let lease_id = CeremonyEventCursorLeaseId::new(uuid::Uuid::new_v4().to_string())?;
            let Some(lease) = self
                .cursors
                .lease(&consumer, lease_id, self.clock.now(), LEASE_DURATION)
                .await?
            else {
                round.busy = true;
                break;
            };
            let Some(positioned) = self
                .events
                .read_all(lease.next_position(), one)
                .await?
                .into_iter()
                .next()
            else {
                self.cursors.release(&lease).await?;
                break;
            };
            if lease.attempt().is_exhausted(MAX_ATTEMPTS) {
                self.cursors
                    .quarantine(
                        &lease,
                        positioned.position,
                        CeremonyEventQuarantineReason::new(
                            "attention projection failed three times",
                        )?,
                        self.clock.now(),
                    )
                    .await?;
                round.read += 1;
                continue;
            }
            if let Err(error) = self.offer(audience, &positioned, &mut round).await {
                tracing::warn!(
                    consumer = consumer.as_str(),
                    ceremony_id = %positioned.record.ceremony_id(),
                    position = ?positioned.position,
                    %error,
                    "attention could not be offered; the position will be read again"
                );
                self.cursors
                    .mark_failed(&lease, positioned.position)
                    .await?;
                round.failed += 1;
                break;
            }
            self.cursors
                .acknowledge_lease(&lease, positioned.position)
                .await?;
            round.read += 1;
        }
        Ok(round)
    }

    /// Read one record as this integrator's business, or as nothing.
    async fn offer(
        &self,
        audience: &AttentionAudience,
        positioned: &PositionedRecord,
        round: &mut ProjectionRound,
    ) -> Result<(), DomainError> {
        if !audience.covers(positioned.record.ceremony_id()) {
            return Ok(());
        }
        let Some(attention) = attention_for(positioned, audience.binding().role_id())? else {
            return Ok(());
        };
        if !audience.policy().admits(attention.kind()) {
            return Ok(());
        }
        let shed = AttentionBackpressure::new(self.deliveries.as_ref())
            .make_room(audience, self.clock.now())
            .await?;
        if shed > 0 {
            round.shed += shed;
            // The host is told it is behind before it is told the news
            // that pushed it over, so the two arrive in the order they
            // happened rather than in the order they were derived.
            self.enqueue(audience, &queue_overflow(positioned)?, round)
                .await?;
        }
        self.enqueue(
            audience,
            &attention.within_execution(audience.system_execution_id().cloned()),
            round,
        )
        .await
    }

    /// Offer one attention event to the bound destination.
    async fn enqueue(
        &self,
        audience: &AttentionAudience,
        attention: &AttentionEvent,
        round: &mut ProjectionRound,
    ) -> Result<(), DomainError> {
        let item =
            HostDeliveryItem::attention(attention.ceremony_id().clone(), attention.id().clone());
        let policy = match audience.binding().destination().activation() {
            HostActivationMode::None => HostDeliveryPolicy::pull(),
            HostActivationMode::Command => HostDeliveryPolicy::activation(),
        };
        let offered = HostDeliveryRecord::queued(
            item,
            audience.binding().delivery_target(),
            policy,
            self.clock.now(),
        )?;
        match self.deliveries.enqueue(offered).await? {
            EnqueueOutcome::AlreadyQueued(_) => {
                round.already_held += 1;
                Ok(())
            }
            EnqueueOutcome::Enqueued(record) => {
                round.queued += 1;
                self.wake(audience, &record, attention, round).await
            }
        }
    }

    /// Reach the host, if this destination is one that can be reached.
    ///
    /// Whatever the adapter answers is written down, including that it
    /// does not wake hosts at all: an operator looking at a delivery
    /// that never moved needs to know which of the two silences it is.
    async fn wake(
        &self,
        audience: &AttentionAudience,
        record: &HostDeliveryRecord,
        attention: &AttentionEvent,
        round: &mut ProjectionRound,
    ) -> Result<(), DomainError> {
        let binding = audience.binding();
        if binding.destination().activation() == HostActivationMode::None {
            return Ok(());
        }
        let envelope = envelope_for(record, binding.id().clone(), binding.fence(), attention);
        let outcome = self.activation.activate(binding, record, &envelope).await?;
        let reached = matches!(outcome, HostActivationOutcome::Accepted(_));
        let failed = matches!(outcome, HostActivationOutcome::Failed(_));
        self.deliveries
            .record_activation(record.id(), &outcome, self.clock.now())
            .await?;
        if reached {
            round.activated += 1;
        }
        if failed {
            round.failed += 1;
        }
        Ok(())
    }
}

/// What travels to the host: why it was woken, and nothing else.
///
/// The context is deliberately thin. An envelope is a knock on the
/// door, and the integrator revalidates the instance before acting, so
/// carrying a snapshot of the ceremony here would be carrying something
/// already stale by the time anybody read it.
fn envelope_for(
    record: &HostDeliveryRecord,
    binding_id: made_core::value_objects::IntegratorBindingId,
    fence: made_core::value_objects::IntegratorFence,
    attention: &AttentionEvent,
) -> HostActivationEnvelope {
    let envelope = HostActivationEnvelope::new(
        record.id().clone(),
        binding_id,
        fence,
        record.item().clone(),
        attention.ceremony_id().clone(),
        attention.kind(),
        attention.reason().clone(),
        attention.occurred_at(),
    )
    .about(attention.summary())
    .with_evidence(attention.evidence().to_vec())
    .caused_by(
        attention.correlation_id().cloned(),
        attention.causation_id().cloned(),
    );
    match attention.step_id() {
        Some(step_id) => envelope.at_step(step_id.clone()),
        None => envelope,
    }
}

#[cfg(test)]
mod tests;
