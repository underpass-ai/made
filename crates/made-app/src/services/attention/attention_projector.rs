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

use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::entities::CeremonyDefinition;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyEventCursorPort, CeremonyEventStorePort, ClockPort, EnqueueOutcome,
    HostActivationOutcome, HostActivationPort, HostDeliveryLedgerPort, PositionedRecord,
};
use made_core::value_objects::{
    CeremonyEventCursorLeaseId, CeremonyEventPageLimit, CeremonyEventQuarantineReason, CeremonyId,
    DurationMs, HostActivationEnvelope, HostActivationMode, HostDeliveryItem, HostDeliveryPolicy,
    HostDeliveryRecord,
};

use super::{
    attention_for, human_decisions_requested, moves_the_session, queue_overflow, AttentionAudience,
    AttentionBackpressure, AttentionEvent, BindingDeliveries, CeremonyDefinitionLookup,
    LoopRoundTally, NoProgressDetector, ProjectionPass, ProjectionRound,
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
    definitions: Arc<dyn CeremonyDefinitionLookup>,
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
        definitions: Arc<dyn CeremonyDefinitionLookup>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            events,
            cursors,
            deliveries,
            activation,
            definitions,
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
        // One walk of this binding's ledger for the whole round,
        // carried forward as the round changes it. The two questions
        // asked of it — is the queue full, has the loop used up its
        // rounds — were each paying for their own walk of up to twenty
        // pages, per record.
        let mut held = BindingDeliveries::new(self.deliveries.as_ref())
            .all(&audience.binding().delivery_target())
            .await?;
        let withholding = NoProgressDetector::new(audience.policy().limits())
            .detect(LoopRoundTally::of(audience.binding().progress()))
            .is_some_and(|stall| !stall.admits_new_results());
        // One resolution per ceremony per round. A round walks up to
        // two hundred positions and a busy ceremony fills most of
        // them; resolving the same definition two hundred times would
        // fold the same session two hundred times.
        let mut definitions = BTreeMap::new();
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
            if let Err(error) = self
                .offer(
                    audience,
                    &positioned,
                    &mut ProjectionPass {
                        held: &mut held,
                        definitions: &mut definitions,
                        withholding,
                        tally: &mut round,
                    },
                )
                .await
            {
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
    ///
    /// One record can be two pieces of news: what the rules read in it,
    /// and — when it moved the session somewhere only a person can take
    /// it out of — one request per guard the visit now waits on.
    async fn offer(
        &self,
        audience: &AttentionAudience,
        positioned: &PositionedRecord,
        round: &mut ProjectionPass<'_>,
    ) -> Result<(), DomainError> {
        if !audience.covers(positioned.record.ceremony_id()) {
            return Ok(());
        }
        let mut readings = Vec::new();
        if let Some(attention) = attention_for(positioned, audience.binding().role_id())? {
            readings.push(attention);
        }
        readings.extend(self.human_decisions(positioned, round.definitions).await?);
        for attention in readings {
            self.offer_one(audience, positioned, attention, round)
                .await?;
        }
        Ok(())
    }

    /// The readings that need the definition, or none when this build
    /// cannot resolve it.
    ///
    /// A ceremony whose definition will not resolve produces no
    /// request, which is the same answer the read path derives later.
    /// The two agreeing is what keeps a delivery from being offered and
    /// then dropped as underivable.
    async fn human_decisions(
        &self,
        positioned: &PositionedRecord,
        resolved: &mut BTreeMap<CeremonyId, Option<CeremonyDefinition>>,
    ) -> Result<Vec<AttentionEvent>, DomainError> {
        // The cheap question first. Most of the feed is steps being
        // claimed and completed, and none of those can produce this
        // reading, so none of them should pay for a definition.
        if !moves_the_session(positioned) {
            return Ok(Vec::new());
        }
        let ceremony_id = positioned.record.ceremony_id();
        if !resolved.contains_key(ceremony_id) {
            let resolution = self.definitions.definition_of(ceremony_id).await?;
            resolved.insert(ceremony_id.clone(), resolution);
        }
        let definition = resolved.get(ceremony_id).cloned().flatten();
        definition
            .map(|definition| human_decisions_requested(positioned, &definition))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    async fn offer_one(
        &self,
        audience: &AttentionAudience,
        positioned: &PositionedRecord,
        attention: AttentionEvent,
        round: &mut ProjectionPass<'_>,
    ) -> Result<(), DomainError> {
        if !audience.policy().admits(attention.kind()) {
            return Ok(());
        }
        if round.withholding && !attention.kind().is_blocking() {
            // A loop that has gone round as many times as it was
            // allowed is not offered more results. Nothing is lost:
            // the cursor advances, and a later binding with rounds
            // left projects the same records again from its own
            // cursor.
            round.tally.withheld += 1;
            return Ok(());
        }
        let shed = AttentionBackpressure::new(self.deliveries.as_ref())
            .make_room(audience, self.clock.now(), round.held)
            .await?;
        if shed > 0 {
            round.tally.shed += shed;
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
        round: &mut ProjectionPass<'_>,
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
                round.tally.already_held += 1;
                Ok(())
            }
            EnqueueOutcome::Enqueued(record) => {
                round.tally.queued += 1;
                // The round's own view of the queue grows with it, so
                // the next record is weighed against what this one
                // just added rather than against a stale walk.
                round.held.push(record.clone());
                self.wake(audience, &record, attention, round.tally).await
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
