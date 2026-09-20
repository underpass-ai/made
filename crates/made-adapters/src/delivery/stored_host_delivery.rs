use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, DeliveryFailureOutcome, HostActivationOutcome, ProcessedOutcome, RecordedActivation,
};
use made_core::value_objects::{
    DeliveryExpiryCause, DeliveryFailureReason, DurationMs, HostAgentIncarnation, HostDeliveryId,
    HostDeliveryLease, HostDeliveryLeaseId, HostDeliveryObservation, HostDeliveryRecord,
    HostDeliveryState, HostDeliveryStateKind, IntegratorFence, ProcessedActionRef,
};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

/// One delivery as a store holds it, with the two facts the domain
/// record deliberately does not carry.
///
/// Who last held it and which integrator generation closed it are
/// storage-side fencing, not part of what the delivery *is*: a host
/// reading its own record has no business being told which other
/// process was there before it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StoredHostDelivery {
    record: HostDeliveryRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owner: Option<HostAgentIncarnation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fence: Option<IntegratorFence>,
}

impl StoredHostDelivery {
    pub(crate) const fn new(record: HostDeliveryRecord) -> Self {
        Self {
            record,
            owner: None,
            fence: None,
        }
    }

    pub(crate) const fn record(&self) -> &HostDeliveryRecord {
        &self.record
    }

    pub(crate) fn into_record(self) -> HostDeliveryRecord {
        self.record
    }

    pub(crate) const fn id(&self) -> &HostDeliveryId {
        self.record.id()
    }

    /// The key the destination index files this delivery under.
    pub(crate) fn target_key(&self) -> String {
        self.record.target().target_key().to_string()
    }

    pub(crate) fn is_offerable_at(&self, now: OffsetDateTime) -> bool {
        self.record.is_offerable_at(now)
    }

    /// Hand this delivery to one host for a bounded time.
    pub(crate) fn leased(
        &self,
        lease_id: HostDeliveryLeaseId,
        owner: &HostAgentIncarnation,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> (Self, HostDeliveryLease) {
        let lease = HostDeliveryLease::new(
            self.record.id().clone(),
            lease_id,
            owner.clone(),
            now + millis(duration),
        );
        let next = Self {
            record: self.record.leased(lease.clone(), now),
            owner: Some(owner.clone()),
            fence: self.fence,
        };
        (next, lease)
    }

    /// Record what the host said, or say why the report is not taken.
    pub(crate) fn acknowledged(
        &self,
        lease: &HostDeliveryLease,
        observation: &HostDeliveryObservation,
        now: OffsetDateTime,
    ) -> (Option<Self>, AckOutcome) {
        if let HostDeliveryState::Acknowledged { observation: held } = self.record.state() {
            return if held == observation {
                (None, AckOutcome::AlreadyAcknowledged(self.record.clone()))
            } else {
                (
                    None,
                    AckOutcome::Conflict {
                        existing: Box::new(held.clone()),
                    },
                )
            };
        }
        if !self.holds(lease, now) {
            return (None, AckOutcome::LeaseNotOwned);
        }
        let next = self.with_record(self.record.acknowledged(observation.clone(), now));
        let outcome = AckOutcome::Acknowledged(next.record.clone());
        (Some(next), outcome)
    }

    /// Close a delivery the host has acted on, or say why not.
    pub(crate) fn processed(
        &self,
        owner: &HostAgentIncarnation,
        fence: Option<IntegratorFence>,
        action: &ProcessedActionRef,
        now: OffsetDateTime,
    ) -> (Option<Self>, ProcessedOutcome) {
        // The fence first: a host that was replaced has no standing to
        // close anything, and finding that out only after the state
        // check would let a stale caller conflict with its successor.
        if let (Some(offered), Some(current)) = (fence, self.fence) {
            if offered < current {
                return (None, ProcessedOutcome::FenceRejected { current });
            }
        }
        if let HostDeliveryState::Processed { action: held, .. } = self.record.state() {
            return if held == action {
                (
                    None,
                    ProcessedOutcome::AlreadyProcessed(self.record.clone()),
                )
            } else {
                (
                    None,
                    ProcessedOutcome::Conflict {
                        existing: Box::new(held.clone()),
                    },
                )
            };
        }
        let state = self.record.state().kind();
        if state != HostDeliveryStateKind::Acknowledged {
            return (None, ProcessedOutcome::NotAcknowledged { state });
        }
        if self.owner.as_ref().is_some_and(|held| held != owner) {
            return (None, ProcessedOutcome::LeaseNotOwned);
        }
        let next = Self {
            record: self.record.processed(action.clone(), now),
            owner: self.owner.clone(),
            fence: fence.or(self.fence),
        };
        let outcome = ProcessedOutcome::Processed(next.record.clone());
        (Some(next), outcome)
    }

    /// Write down what an activation adapter did.
    ///
    /// A delivered record is not a closed one: the receipt says a host
    /// was reached, which is a claim by the transport and not by the
    /// host, so the delivery stays offerable and somebody still has to
    /// come and take it. A wake-up that failed costs an attempt like
    /// any other, because a host that cannot be reached twice is the
    /// same problem as one that never answers.
    pub(crate) fn activated(
        &self,
        outcome: &HostActivationOutcome,
        now: OffsetDateTime,
    ) -> (Option<Self>, RecordedActivation) {
        if self.record.state().is_terminal() {
            return (None, RecordedActivation::AlreadyEnded(self.record.clone()));
        }
        match outcome {
            HostActivationOutcome::Unsupported => {
                (None, RecordedActivation::NotAttempted(self.record.clone()))
            }
            HostActivationOutcome::Accepted(receipt) => {
                let next = self.with_record(self.record.delivered(receipt.clone(), now));
                let recorded = RecordedActivation::Delivered(next.record.clone());
                (Some(next), recorded)
            }
            HostActivationOutcome::Failed(reason) => {
                let attempted = self.record.attempt().next();
                if self
                    .record
                    .policy()
                    .max_attempts()
                    .is_exhausted_by(attempted)
                {
                    let next = self.with_record(self.record.failed(reason.clone(), now));
                    let recorded = RecordedActivation::Exhausted(next.record.clone());
                    return (Some(next), recorded);
                }
                let next = self.with_record(self.record.requeued(now));
                let recorded = RecordedActivation::Requeued(next.record.clone());
                (Some(next), recorded)
            }
        }
    }

    /// Count a failed attempt, and retry or give up by the policy.
    pub(crate) fn failed(
        &self,
        lease: &HostDeliveryLease,
        reason: &DeliveryFailureReason,
        now: OffsetDateTime,
    ) -> (Option<Self>, DeliveryFailureOutcome) {
        if !self.holds(lease, now) {
            return (None, DeliveryFailureOutcome::LeaseNotOwned);
        }
        let attempted = self.record.attempt().next();
        if self
            .record
            .policy()
            .max_attempts()
            .is_exhausted_by(attempted)
        {
            let next = self.with_record(self.record.failed(reason.clone(), now));
            let outcome = DeliveryFailureOutcome::Exhausted(next.record.clone());
            return (Some(next), outcome);
        }
        let next = self.with_record(self.record.requeued(now));
        let outcome = DeliveryFailureOutcome::Requeued(next.record.clone());
        (Some(next), outcome)
    }

    /// Give a lease back untouched, if it is the one that holds this.
    pub(crate) fn released(&self, lease: &HostDeliveryLease, now: OffsetDateTime) -> Option<Self> {
        self.holds(lease, now)
            .then(|| self.with_record(self.record.released(now)))
    }

    /// Whatever has run out of time, and nothing else.
    pub(crate) fn expired(&self, now: OffsetDateTime) -> Option<Self> {
        match self.record.state() {
            HostDeliveryState::Leased { lease } if !lease.is_live_at(now) => {
                Some(self.with_record(self.record.released(now)))
            }
            HostDeliveryState::Acknowledged { observation } => {
                let timeout = self.record.policy().ack_timeout()?;
                (observation.observed_at() + millis(timeout) <= now).then(|| {
                    self.with_record(self.record.expired(DeliveryExpiryCause::Timeout, now))
                })
            }
            _ => None,
        }
    }

    /// Give up on a delivery that is still open, naming why.
    ///
    /// Distinct from the timeout sweep: nothing here has run out of
    /// its own time, something outside has made the offer pointless.
    /// A terminal record is left alone, because an offer that was
    /// answered, refused or exhausted already has an ending and
    /// overwriting it would erase what happened.
    pub(crate) fn abandoned(
        &self,
        cause: DeliveryExpiryCause,
        now: OffsetDateTime,
    ) -> Option<Self> {
        (!self.record.state().is_terminal())
            .then(|| self.with_record(self.record.expired(cause, now)))
    }

    /// Close this delivery because its destination was replaced.
    pub(crate) fn superseded(&self, by: Option<HostDeliveryId>, now: OffsetDateTime) -> Self {
        self.with_record(self.record.superseded(by, now))
    }

    /// The same item, offered to the destination that replaced this one.
    pub(crate) fn re_addressed(
        &self,
        target: made_core::value_objects::HostDeliveryTarget,
        now: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        self.record.re_addressed(target, now).map(Self::new)
    }

    fn holds(&self, lease: &HostDeliveryLease, now: OffsetDateTime) -> bool {
        self.record
            .live_lease_at(now)
            .is_some_and(|held| held.lease_id() == lease.lease_id())
    }

    fn with_record(&self, record: HostDeliveryRecord) -> Self {
        Self {
            record,
            owner: self.owner.clone(),
            fence: self.fence,
        }
    }
}

fn millis(duration: DurationMs) -> Duration {
    Duration::milliseconds(i64::try_from(duration.get()).unwrap_or(i64::MAX))
}
