use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::DomainError;

use super::{
    DeliveryAttempt, DeliveryExpiryCause, DeliveryFailureReason, DeliveryHistory,
    DeliveryHistoryEntry, HostActivationReceipt, HostDeliveryId, HostDeliveryItem,
    HostDeliveryLease, HostDeliveryObservation, HostDeliveryPolicy, HostDeliveryState,
    HostDeliveryStateKind, HostDeliveryTarget, ProcessedActionRef,
};

/// One item's journey to one host destination, as the ledger holds it.
///
/// Immutable: every transition returns a new record. The ledger decides
/// whether a transition is allowed and writes the result, so the rules
/// about what may follow what live in one place and an adapter cannot
/// invent a shortcut through them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostDeliveryRecord {
    id: HostDeliveryId,
    item: HostDeliveryItem,
    target: HostDeliveryTarget,
    policy: HostDeliveryPolicy,
    state: HostDeliveryState,
    attempt: DeliveryAttempt,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
    history: DeliveryHistory,
}

impl HostDeliveryRecord {
    /// A new delivery, queued, with its identity derived from what it is.
    pub fn queued(
        item: HostDeliveryItem,
        target: HostDeliveryTarget,
        policy: HostDeliveryPolicy,
        created_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        let id = item.delivery_id(&target)?;
        Ok(Self {
            id,
            item,
            target,
            policy,
            state: HostDeliveryState::Queued,
            attempt: DeliveryAttempt::NONE,
            created_at,
            updated_at: created_at,
            history: DeliveryHistory::new().recording(DeliveryHistoryEntry::new(
                HostDeliveryStateKind::Queued,
                created_at,
            )),
        })
    }

    #[must_use]
    pub const fn id(&self) -> &HostDeliveryId {
        &self.id
    }

    #[must_use]
    pub const fn item(&self) -> &HostDeliveryItem {
        &self.item
    }

    #[must_use]
    pub const fn target(&self) -> &HostDeliveryTarget {
        &self.target
    }

    #[must_use]
    pub const fn policy(&self) -> &HostDeliveryPolicy {
        &self.policy
    }

    #[must_use]
    pub const fn state(&self) -> &HostDeliveryState {
        &self.state
    }

    #[must_use]
    pub const fn attempt(&self) -> DeliveryAttempt {
        self.attempt
    }

    #[must_use]
    pub const fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    #[must_use]
    pub const fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    #[must_use]
    pub const fn history(&self) -> &DeliveryHistory {
        &self.history
    }

    /// Whether a host could be handed this delivery at `now`.
    ///
    /// A live lease excludes everyone else; an expired one does not, and
    /// that is the whole of the exclusion rule. A delivery an activation
    /// adapter already pushed stays offerable, because reaching a host
    /// is not the same as the host having answered.
    #[must_use]
    pub fn is_offerable_at(&self, now: OffsetDateTime) -> bool {
        match &self.state {
            HostDeliveryState::Queued | HostDeliveryState::DeliveredToHost { .. } => true,
            HostDeliveryState::Leased { lease } => !lease.is_live_at(now),
            _ => false,
        }
    }

    /// The lease currently excluding other holders, if one still does.
    #[must_use]
    pub fn live_lease_at(&self, now: OffsetDateTime) -> Option<&HostDeliveryLease> {
        self.state.lease().filter(|lease| lease.is_live_at(now))
    }

    /// Handed out exclusively.
    #[must_use]
    pub fn leased(&self, lease: HostDeliveryLease, now: OffsetDateTime) -> Self {
        self.transitioned(HostDeliveryState::Leased { lease }, now)
    }

    /// Pushed to the host by an activation adapter.
    #[must_use]
    pub fn delivered(&self, receipt: HostActivationReceipt, now: OffsetDateTime) -> Self {
        self.transitioned(HostDeliveryState::DeliveredToHost { receipt }, now)
    }

    /// The host said what it saw.
    #[must_use]
    pub fn acknowledged(&self, observation: HostDeliveryObservation, now: OffsetDateTime) -> Self {
        self.transitioned(HostDeliveryState::Acknowledged { observation }, now)
    }

    /// The host acted, and named the act.
    #[must_use]
    pub fn processed(&self, action: ProcessedActionRef, now: OffsetDateTime) -> Self {
        self.transitioned(HostDeliveryState::Processed { at: now, action }, now)
    }

    /// Given up on, with the attempt that gave up counted.
    #[must_use]
    pub fn failed(&self, reason: DeliveryFailureReason, now: OffsetDateTime) -> Self {
        let attempt = self.attempt.next();
        let mut next = self.transitioned(HostDeliveryState::Failed { reason, attempt }, now);
        next.attempt = attempt;
        next
    }

    /// Back in the queue after a failed attempt, which is counted.
    #[must_use]
    pub fn requeued(&self, now: OffsetDateTime) -> Self {
        let mut next = self.transitioned(HostDeliveryState::Queued, now);
        next.attempt = self.attempt.next();
        next
    }

    /// Back in the queue because a holder gave up its lease, uncounted.
    ///
    /// Releasing is not failing: a host that hands work back untouched
    /// has not used up one of the delivery's chances.
    #[must_use]
    pub fn released(&self, now: OffsetDateTime) -> Self {
        self.transitioned(HostDeliveryState::Queued, now)
    }

    /// No longer worth making.
    #[must_use]
    pub fn expired(&self, cause: DeliveryExpiryCause, now: OffsetDateTime) -> Self {
        self.transitioned(HostDeliveryState::Expired { at: now, cause }, now)
    }

    /// Closed because its destination was replaced.
    ///
    /// `by` is the delivery opened in its place, when the work follows.
    #[must_use]
    pub fn superseded(&self, by: Option<HostDeliveryId>, now: OffsetDateTime) -> Self {
        self.transitioned(HostDeliveryState::Superseded { by }, now)
    }

    /// The same delivery offered to a different destination.
    ///
    /// A new identity, because the destination is part of what a
    /// delivery *is*: the replacement can be superseded, acknowledged
    /// and counted on its own without disturbing the record it follows.
    pub fn re_addressed(
        &self,
        target: HostDeliveryTarget,
        now: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        Self::queued(self.item.clone(), target, self.policy.clone(), now)
    }

    fn transitioned(&self, state: HostDeliveryState, now: OffsetDateTime) -> Self {
        let kind = state.kind();
        Self {
            state,
            updated_at: now,
            history: self
                .history
                .clone()
                .recording(DeliveryHistoryEntry::new(kind, now)),
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{
        CeremonyId, CeremonyInterventionId, HostAgentIncarnation, HostDeliveryLeaseId, RoleId,
    };
    use time::Duration;

    fn record() -> HostDeliveryRecord {
        HostDeliveryRecord::queued(
            HostDeliveryItem::intervention(
                CeremonyId::new("c-1").unwrap(),
                CeremonyInterventionId::new("i-1").unwrap(),
            ),
            HostDeliveryTarget::role(RoleId::new("ENGINEER").unwrap()),
            HostDeliveryPolicy::default(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    fn lease(until: OffsetDateTime) -> HostDeliveryLease {
        HostDeliveryLease::new(
            record().id().clone(),
            HostDeliveryLeaseId::new("l-1").unwrap(),
            HostAgentIncarnation::new("run-7").unwrap(),
            until,
        )
    }

    #[test]
    fn a_live_lease_excludes_and_an_expired_one_does_not() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let leased = record().leased(lease(now + Duration::seconds(60)), now);
        assert!(!leased.is_offerable_at(now));
        assert!(leased.live_lease_at(now).is_some());
        assert!(leased.is_offerable_at(now + Duration::seconds(61)));
        assert!(leased.live_lease_at(now + Duration::seconds(61)).is_none());
    }

    #[test]
    fn failing_counts_an_attempt_and_releasing_does_not() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let reason = DeliveryFailureReason::new("the host was busy").unwrap();
        assert_eq!(record().failed(reason, now).attempt().value(), 1);
        assert_eq!(record().requeued(now).attempt().value(), 1);
        assert_eq!(record().released(now).attempt().value(), 0);
    }

    #[test]
    fn re_addressing_produces_a_delivery_of_its_own() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let original = record();
        let moved = original
            .re_addressed(
                HostDeliveryTarget::role(RoleId::new("REVIEWER").unwrap()),
                now,
            )
            .unwrap();
        assert_ne!(original.id(), moved.id());
        assert_eq!(original.item(), moved.item());
        assert_eq!(moved.state().kind(), HostDeliveryStateKind::Queued);
    }

    #[test]
    fn every_transition_is_remembered_in_order() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let processed = record()
            .leased(lease(now + Duration::seconds(1)), now)
            .acknowledged(
                HostDeliveryObservation::new(
                    crate::value_objects::HostDeliveryObservationKind::Received,
                    now,
                    None,
                    crate::value_objects::DeliveryNote::new("taken").unwrap(),
                ),
                now,
            )
            .processed(
                ProcessedActionRef::of(crate::value_objects::ProcessedActionKind::Responded),
                now,
            );
        let kinds: Vec<_> = processed
            .history()
            .entries()
            .iter()
            .map(|entry| entry.state_kind())
            .collect();
        assert_eq!(
            kinds,
            vec![
                HostDeliveryStateKind::Queued,
                HostDeliveryStateKind::Leased,
                HostDeliveryStateKind::Acknowledged,
                HostDeliveryStateKind::Processed,
            ]
        );
    }
}
