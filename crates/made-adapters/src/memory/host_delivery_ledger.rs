use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, DeliveryFailureOutcome, EnqueueOutcome, HostActivationOutcome, HostDeliveryFilter,
    HostDeliveryLedgerPort, HostDeliveryPage, HostDeliveryPageLimit, HostDeliveryQuery,
    LeasedDelivery, ProcessedOutcome, RecordedActivation, SupersessionOutcome,
};
use made_core::value_objects::{
    CeremonyId, DeliveryExpiryCause, DeliveryFailureReason, DurationMs, FollowReplacement,
    HostAgentIncarnation, HostDeliveryId, HostDeliveryLease, HostDeliveryLeaseId,
    HostDeliveryObservation, HostDeliveryRecord, HostDeliveryTarget, IntegratorFence,
    ProcessedActionRef,
};
use time::OffsetDateTime;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::delivery::StoredHostDelivery;

/// Process-local host-delivery ledger.
///
/// A sorted map because the contract's paging is by delivery identity:
/// an unordered store would hand a caller the same page twice and call
/// it progress.
#[derive(Debug, Default, Clone)]
pub struct InMemoryHostDeliveryLedger {
    inner: Arc<RwLock<BTreeMap<HostDeliveryId, StoredHostDelivery>>>,
}

impl InMemoryHostDeliveryLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl HostDeliveryLedgerPort for InMemoryHostDeliveryLedger {
    async fn enqueue(&self, record: HostDeliveryRecord) -> Result<EnqueueOutcome, DomainError> {
        let mut deliveries = self.inner.write().await;
        if let Some(existing) = deliveries.get(record.id()) {
            return Ok(EnqueueOutcome::AlreadyQueued(existing.record().clone()));
        }
        let id = record.id().clone();
        let accepted = StoredHostDelivery::new(record);
        let answer = accepted.record().clone();
        deliveries.insert(id, accepted);
        Ok(EnqueueOutcome::Enqueued(answer))
    }

    async fn lease(
        &self,
        filter: &HostDeliveryFilter,
        owner: &HostAgentIncarnation,
        now: OffsetDateTime,
        duration: DurationMs,
        limit: HostDeliveryPageLimit,
    ) -> Result<Vec<LeasedDelivery>, DomainError> {
        let mut deliveries = self.inner.write().await;
        let offerable: Vec<HostDeliveryId> = deliveries
            .values()
            .filter(|stored| stored.is_offerable_at(now) && filter.admits(stored.record()))
            .take(limit.as_usize())
            .map(|stored| stored.id().clone())
            .collect();

        let mut leased = Vec::with_capacity(offerable.len());
        for id in offerable {
            let Some(stored) = deliveries.get(&id) else {
                continue;
            };
            let (next, lease) = stored.leased(lease_id()?, owner, now, duration);
            leased.push(LeasedDelivery::new(lease, next.record().clone()));
            deliveries.insert(id, next);
        }
        Ok(leased)
    }

    async fn acknowledge(
        &self,
        lease: &HostDeliveryLease,
        observation: &HostDeliveryObservation,
        now: OffsetDateTime,
    ) -> Result<AckOutcome, DomainError> {
        let mut deliveries = self.inner.write().await;
        let Some(stored) = deliveries.get(lease.delivery_id()) else {
            return Ok(AckOutcome::LeaseNotOwned);
        };
        let (next, outcome) = stored.acknowledged(lease, observation, now);
        if let Some(next) = next {
            deliveries.insert(lease.delivery_id().clone(), next);
        }
        Ok(outcome)
    }

    async fn mark_processed(
        &self,
        delivery_id: &HostDeliveryId,
        owner: &HostAgentIncarnation,
        fence: Option<IntegratorFence>,
        action: &ProcessedActionRef,
        now: OffsetDateTime,
    ) -> Result<ProcessedOutcome, DomainError> {
        let mut deliveries = self.inner.write().await;
        let stored = deliveries.get(delivery_id).ok_or(DomainError::NotFound {
            what: "host_delivery",
        })?;
        let (next, outcome) = stored.processed(owner, fence, action, now);
        if let Some(next) = next {
            deliveries.insert(delivery_id.clone(), next);
        }
        Ok(outcome)
    }

    async fn record_activation(
        &self,
        delivery_id: &HostDeliveryId,
        outcome: &HostActivationOutcome,
        now: OffsetDateTime,
    ) -> Result<RecordedActivation, DomainError> {
        let mut deliveries = self.inner.write().await;
        let Some(stored) = deliveries.get(delivery_id) else {
            return Ok(RecordedActivation::Unknown);
        };
        let (next, recorded) = stored.activated(outcome, now);
        if let Some(next) = next {
            deliveries.insert(delivery_id.clone(), next);
        }
        Ok(recorded)
    }

    async fn mark_failed(
        &self,
        lease: &HostDeliveryLease,
        reason: &DeliveryFailureReason,
        now: OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, DomainError> {
        let mut deliveries = self.inner.write().await;
        let Some(stored) = deliveries.get(lease.delivery_id()) else {
            return Ok(DeliveryFailureOutcome::LeaseNotOwned);
        };
        let (next, outcome) = stored.failed(lease, reason, now);
        if let Some(next) = next {
            deliveries.insert(lease.delivery_id().clone(), next);
        }
        Ok(outcome)
    }

    async fn release(
        &self,
        lease: &HostDeliveryLease,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut deliveries = self.inner.write().await;
        if let Some(next) = deliveries
            .get(lease.delivery_id())
            .and_then(|stored| stored.released(lease, now))
        {
            deliveries.insert(lease.delivery_id().clone(), next);
        }
        Ok(())
    }

    async fn expire(&self, now: OffsetDateTime) -> Result<Vec<HostDeliveryId>, DomainError> {
        let mut deliveries = self.inner.write().await;
        let expired: Vec<(HostDeliveryId, StoredHostDelivery)> = deliveries
            .values()
            .filter_map(|stored| stored.expired(now).map(|next| (stored.id().clone(), next)))
            .collect();
        let ids = expired.iter().map(|(id, _)| id.clone()).collect();
        for (id, next) in expired {
            deliveries.insert(id, next);
        }
        Ok(ids)
    }

    async fn abandon(
        &self,
        delivery_id: &HostDeliveryId,
        cause: DeliveryExpiryCause,
        now: OffsetDateTime,
    ) -> Result<Option<HostDeliveryRecord>, DomainError> {
        let mut deliveries = self.inner.write().await;
        let Some(next) = deliveries
            .get(delivery_id)
            .and_then(|stored| stored.abandoned(cause, now))
        else {
            return Ok(None);
        };
        let record = next.record().clone();
        deliveries.insert(delivery_id.clone(), next);
        Ok(Some(record))
    }

    async fn expire_ceremony(
        &self,
        ceremony_id: &CeremonyId,
        cause: DeliveryExpiryCause,
        now: OffsetDateTime,
    ) -> Result<Vec<HostDeliveryId>, DomainError> {
        let mut deliveries = self.inner.write().await;
        let abandoned: Vec<(HostDeliveryId, StoredHostDelivery)> = deliveries
            .values()
            .filter(|stored| stored.record().item().ceremony_id() == ceremony_id)
            .filter_map(|stored| {
                stored
                    .abandoned(cause, now)
                    .map(|next| (stored.id().clone(), next))
            })
            .collect();
        let ids = abandoned.iter().map(|(id, _)| id.clone()).collect();
        for (id, next) in abandoned {
            deliveries.insert(id, next);
        }
        Ok(ids)
    }

    async fn supersede(
        &self,
        previous: &HostDeliveryTarget,
        replacement: &HostDeliveryTarget,
        follow: FollowReplacement,
        now: OffsetDateTime,
    ) -> Result<SupersessionOutcome, DomainError> {
        let mut deliveries = self.inner.write().await;
        let affected: Vec<StoredHostDelivery> = deliveries
            .values()
            .filter(|stored| {
                stored.record().target() == previous && !stored.record().state().is_terminal()
            })
            .cloned()
            .collect();

        let mut superseded = Vec::with_capacity(affected.len());
        let mut replacements = Vec::new();
        for stored in affected {
            let opened = if follow.follows() {
                let candidate = stored.re_addressed(replacement.clone(), now)?;
                let id = candidate.id().clone();
                if !deliveries.contains_key(&id) {
                    replacements.push(candidate.record().clone());
                    deliveries.insert(id.clone(), candidate);
                }
                Some(id)
            } else {
                None
            };
            superseded.push(stored.id().clone());
            deliveries.insert(stored.id().clone(), stored.superseded(opened, now));
        }
        Ok(SupersessionOutcome::new(superseded, replacements))
    }

    async fn get(&self, id: &HostDeliveryId) -> Result<Option<HostDeliveryRecord>, DomainError> {
        Ok(self
            .inner
            .read()
            .await
            .get(id)
            .map(|stored| stored.record().clone()))
    }

    async fn list(&self, query: &HostDeliveryQuery) -> Result<HostDeliveryPage, DomainError> {
        let deliveries = self.inner.read().await;
        let mut matching = deliveries
            .values()
            .filter(|stored| query.admits(stored.record()))
            .filter(|stored| query.cursor().is_none_or(|after| stored.id() > after))
            .map(|stored| stored.record().clone());

        let records: Vec<HostDeliveryRecord> =
            matching.by_ref().take(query.limit().as_usize()).collect();
        let next_cursor = matching
            .next()
            .is_some()
            .then(|| records.last().map(|record| record.id().clone()))
            .flatten();
        Ok(HostDeliveryPage::new(records, next_cursor))
    }
}

fn lease_id() -> Result<HostDeliveryLeaseId, DomainError> {
    HostDeliveryLeaseId::new(Uuid::new_v4().to_string())
}
