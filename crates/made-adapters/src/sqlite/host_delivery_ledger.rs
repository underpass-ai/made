//! The host-delivery ledger in the canonical embedded SQLite store.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, DeliveryFailureOutcome, EnqueueOutcome, HostDeliveryFilter, HostDeliveryLedgerPort,
    HostDeliveryPage, HostDeliveryPageLimit, HostDeliveryQuery, LeasedDelivery, ProcessedOutcome,
    SupersessionOutcome,
};
use made_core::value_objects::{
    CeremonyId, DeliveryExpiryCause, DeliveryFailureReason, DurationMs, FollowReplacement,
    HostAgentIncarnation, HostDeliveryId, HostDeliveryLease, HostDeliveryLeaseId,
    HostDeliveryObservation, HostDeliveryRecord, HostDeliveryTarget, IntegratorFence,
    ProcessedActionRef,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::delivery::StoredHostDelivery;
use crate::engine::{Engine, Key, Table, WriteTx};

use super::ceremony_store::{decode, encode};
use super::error::join_failure;
use super::host_delivery_scan::{collect, to_destination};
use super::keys::{host_delivery_ceremony_prefix, host_delivery_target};
use super::SqliteCeremonyStore;

/// Work handed out to hosts, durable beside the streams it is about.
#[derive(Debug, Clone)]
pub struct SqliteHostDeliveryLedger {
    engine: Arc<dyn Engine>,
}

impl SqliteHostDeliveryLedger {
    /// Open the ledger through the canonical store's own lifecycle checks.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        SqliteCeremonyStore::open(path).map(|store| store.host_delivery_ledger())
    }

    pub(super) fn from_engine(engine: Arc<dyn Engine>) -> Self {
        Self { engine }
    }

    async fn blocking<T, F>(&self, op: &'static str, work: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(&dyn Engine) -> Result<T, DomainError> + Send + 'static,
    {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || work(engine.as_ref()))
            .await
            .map_err(|error| join_failure(&error, op))?
    }
}

#[async_trait]
impl HostDeliveryLedgerPort for SqliteHostDeliveryLedger {
    /// The occupant is read and the row written in one transaction, so
    /// two projectors replaying the same record cannot both create it.
    async fn enqueue(&self, record: HostDeliveryRecord) -> Result<EnqueueOutcome, DomainError> {
        self.blocking("enqueue host delivery", move |engine| {
            let mut tx = engine.begin_write()?;
            if let Some(existing) = read(tx.as_ref(), record.id())? {
                return Ok(EnqueueOutcome::AlreadyQueued(existing.into_record()));
            }
            write(tx.as_mut(), &StoredHostDelivery::new(record.clone()))?;
            tx.commit()?;
            Ok(EnqueueOutcome::Enqueued(record))
        })
        .await
    }

    async fn lease(
        &self,
        filter: &HostDeliveryFilter,
        owner: &HostAgentIncarnation,
        now: OffsetDateTime,
        duration: DurationMs,
        limit: HostDeliveryPageLimit,
    ) -> Result<Vec<LeasedDelivery>, DomainError> {
        let filter = filter.clone();
        let owner = owner.clone();
        self.blocking("lease host deliveries", move |engine| {
            let mut tx = engine.begin_write()?;
            let offerable = |stored: &StoredHostDelivery| {
                stored.is_offerable_at(now) && filter.admits(stored.record())
            };
            let candidates = match filter.target().keys() {
                Some(keys) => {
                    let mut found = Vec::new();
                    for key in keys {
                        let remaining = limit.as_usize() - found.len();
                        found.extend(to_destination(
                            tx.as_ref(),
                            key.as_str(),
                            remaining,
                            offerable,
                        )?);
                        if found.len() == limit.as_usize() {
                            break;
                        }
                    }
                    found
                }
                None => collect(
                    tx.as_ref(),
                    Table::HostDeliveries,
                    None,
                    None,
                    limit.as_usize(),
                    offerable,
                )?,
            };

            let mut leased = Vec::with_capacity(candidates.len());
            for stored in candidates {
                let (next, lease) = stored.leased(lease_id()?, &owner, now, duration);
                leased.push(LeasedDelivery::new(lease, next.record().clone()));
                write(tx.as_mut(), &next)?;
            }
            if !leased.is_empty() {
                tx.commit()?;
            }
            Ok(leased)
        })
        .await
    }

    async fn acknowledge(
        &self,
        lease: &HostDeliveryLease,
        observation: &HostDeliveryObservation,
        now: OffsetDateTime,
    ) -> Result<AckOutcome, DomainError> {
        let lease = lease.clone();
        let observation = observation.clone();
        self.blocking("acknowledge host delivery", move |engine| {
            let mut tx = engine.begin_write()?;
            let Some(stored) = read(tx.as_ref(), lease.delivery_id())? else {
                return Ok(AckOutcome::LeaseNotOwned);
            };
            let (next, outcome) = stored.acknowledged(&lease, &observation, now);
            if let Some(next) = next {
                write(tx.as_mut(), &next)?;
                tx.commit()?;
            }
            Ok(outcome)
        })
        .await
    }

    async fn mark_processed(
        &self,
        delivery_id: &HostDeliveryId,
        owner: &HostAgentIncarnation,
        fence: Option<IntegratorFence>,
        action: &ProcessedActionRef,
        now: OffsetDateTime,
    ) -> Result<ProcessedOutcome, DomainError> {
        let delivery_id = delivery_id.clone();
        let owner = owner.clone();
        let action = action.clone();
        self.blocking("close host delivery", move |engine| {
            let mut tx = engine.begin_write()?;
            let stored = read(tx.as_ref(), &delivery_id)?.ok_or(DomainError::NotFound {
                what: "host_delivery",
            })?;
            let (next, outcome) = stored.processed(&owner, fence, &action, now);
            if let Some(next) = next {
                write(tx.as_mut(), &next)?;
                tx.commit()?;
            }
            Ok(outcome)
        })
        .await
    }

    async fn mark_failed(
        &self,
        lease: &HostDeliveryLease,
        reason: &DeliveryFailureReason,
        now: OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, DomainError> {
        let lease = lease.clone();
        let reason = reason.clone();
        self.blocking("fail host delivery", move |engine| {
            let mut tx = engine.begin_write()?;
            let Some(stored) = read(tx.as_ref(), lease.delivery_id())? else {
                return Ok(DeliveryFailureOutcome::LeaseNotOwned);
            };
            let (next, outcome) = stored.failed(&lease, &reason, now);
            if let Some(next) = next {
                write(tx.as_mut(), &next)?;
                tx.commit()?;
            }
            Ok(outcome)
        })
        .await
    }

    async fn release(
        &self,
        lease: &HostDeliveryLease,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let lease = lease.clone();
        self.blocking("release host delivery", move |engine| {
            let mut tx = engine.begin_write()?;
            let released = read(tx.as_ref(), lease.delivery_id())?
                .and_then(|stored| stored.released(&lease, now));
            if let Some(next) = released {
                write(tx.as_mut(), &next)?;
                tx.commit()?;
            }
            Ok(())
        })
        .await
    }

    async fn expire(&self, now: OffsetDateTime) -> Result<Vec<HostDeliveryId>, DomainError> {
        self.blocking("expire host deliveries", move |engine| {
            let mut tx = engine.begin_write()?;
            let stale = collect(
                tx.as_ref(),
                Table::HostDeliveries,
                None,
                None,
                usize::MAX,
                |stored| stored.expired(now).is_some(),
            )?;
            let mut expired = Vec::with_capacity(stale.len());
            for stored in stale {
                let Some(next) = stored.expired(now) else {
                    continue;
                };
                expired.push(next.id().clone());
                write(tx.as_mut(), &next)?;
            }
            if !expired.is_empty() {
                tx.commit()?;
            }
            Ok(expired)
        })
        .await
    }

    async fn expire_ceremony(
        &self,
        ceremony_id: &CeremonyId,
        cause: DeliveryExpiryCause,
        now: OffsetDateTime,
    ) -> Result<Vec<HostDeliveryId>, DomainError> {
        let prefix = host_delivery_ceremony_prefix(ceremony_id);
        self.blocking("expire a ceremony's host deliveries", move |engine| {
            let mut tx = engine.begin_write()?;
            let open = collect(
                tx.as_ref(),
                Table::HostDeliveries,
                Some(&prefix),
                None,
                usize::MAX,
                |stored| stored.abandoned(cause, now).is_some(),
            )?;
            let mut abandoned = Vec::with_capacity(open.len());
            for stored in open {
                let Some(next) = stored.abandoned(cause, now) else {
                    continue;
                };
                abandoned.push(next.id().clone());
                write(tx.as_mut(), &next)?;
            }
            if !abandoned.is_empty() {
                tx.commit()?;
            }
            Ok(abandoned)
        })
        .await
    }

    async fn supersede(
        &self,
        previous: &HostDeliveryTarget,
        replacement: &HostDeliveryTarget,
        follow: FollowReplacement,
        now: OffsetDateTime,
    ) -> Result<SupersessionOutcome, DomainError> {
        let previous = previous.clone();
        let replacement = replacement.clone();
        self.blocking("supersede host deliveries", move |engine| {
            let mut tx = engine.begin_write()?;
            let affected = to_destination(
                tx.as_ref(),
                previous.target_key().as_str(),
                usize::MAX,
                |stored| {
                    stored.record().target() == &previous && !stored.record().state().is_terminal()
                },
            )?;

            let mut superseded = Vec::with_capacity(affected.len());
            let mut replacements = Vec::new();
            for stored in affected {
                let opened = if follow.follows() {
                    let candidate = stored.re_addressed(replacement.clone(), now)?;
                    let id = candidate.id().clone();
                    if read(tx.as_ref(), &id)?.is_none() {
                        replacements.push(candidate.record().clone());
                        write(tx.as_mut(), &candidate)?;
                    }
                    Some(id)
                } else {
                    None
                };
                superseded.push(stored.id().clone());
                write(tx.as_mut(), &stored.superseded(opened, now))?;
            }
            if !superseded.is_empty() {
                tx.commit()?;
            }
            Ok(SupersessionOutcome::new(superseded, replacements))
        })
        .await
    }

    async fn get(&self, id: &HostDeliveryId) -> Result<Option<HostDeliveryRecord>, DomainError> {
        let id = id.clone();
        self.blocking("read host delivery", move |engine| {
            let tx = engine.begin_read()?;
            Ok(read(tx.as_ref(), &id)?.map(StoredHostDelivery::into_record))
        })
        .await
    }

    async fn list(&self, query: &HostDeliveryQuery) -> Result<HostDeliveryPage, DomainError> {
        let query = query.clone();
        self.blocking("list host deliveries", move |engine| {
            let tx = engine.begin_read()?;
            let prefix = query.ceremony_id().map(host_delivery_ceremony_prefix);
            // One more than asked for: the extra row is how the page
            // knows whether to offer a cursor, and it is dropped before
            // the answer goes out.
            let wanted = query.limit().as_usize() + 1;
            let mut found = collect(
                tx.as_ref(),
                Table::HostDeliveries,
                prefix.as_deref(),
                query.cursor().map(ToString::to_string),
                wanted,
                |stored| query.admits(stored.record()),
            )?;
            let more = found.len() == wanted;
            found.truncate(query.limit().as_usize());
            let records: Vec<HostDeliveryRecord> = found
                .into_iter()
                .map(StoredHostDelivery::into_record)
                .collect();
            let next_cursor = more
                .then(|| records.last().map(|record| record.id().clone()))
                .flatten();
            Ok(HostDeliveryPage::new(records, next_cursor))
        })
        .await
    }
}

impl SqliteCeremonyStore {
    /// The delivery ledger sharing this store's open engine and pool.
    #[must_use]
    pub fn host_delivery_ledger(&self) -> SqliteHostDeliveryLedger {
        SqliteHostDeliveryLedger::from_engine(Arc::clone(&self.engine))
    }
}

fn read(
    tx: &dyn crate::engine::ReadTx,
    id: &HostDeliveryId,
) -> Result<Option<StoredHostDelivery>, DomainError> {
    tx.get(Table::HostDeliveries, Key::Str(id.as_str()))?
        .map(|bytes| decode(&bytes, "decode host delivery"))
        .transpose()
}

/// One delivery and its place in the destination index, together.
///
/// Together because they are one fact: a row whose index entry was
/// written by a later transaction would be invisible to the host it is
/// addressed to for exactly as long as the gap lasted.
fn write(tx: &mut dyn WriteTx, stored: &StoredHostDelivery) -> Result<(), DomainError> {
    let id = stored.id().as_str().to_owned();
    tx.insert(
        Table::HostDeliveries,
        Key::Str(&id),
        &encode(stored, "encode host delivery")?,
    )?;
    let index = host_delivery_target(&stored.target_key(), &id);
    tx.insert(
        Table::HostDeliveryTargets,
        Key::Str(&index),
        &encode(&id, "encode host delivery index")?,
    )
}

fn lease_id() -> Result<HostDeliveryLeaseId, DomainError> {
    HostDeliveryLeaseId::new(Uuid::new_v4().to_string())
}
