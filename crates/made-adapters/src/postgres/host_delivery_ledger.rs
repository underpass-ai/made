//! The host-delivery ledger on Postgres, for the clustered service.

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, DeliveryFailureOutcome, EnqueueOutcome, HostDeliveryFilter, HostDeliveryLedgerPort,
    HostDeliveryPage, HostDeliveryPageLimit, HostDeliveryQuery, LeasedDelivery, ProcessedOutcome,
    SupersessionOutcome,
};
use made_core::value_objects::{
    DeliveryFailureReason, DurationMs, FollowReplacement, HostAgentIncarnation, HostDeliveryId,
    HostDeliveryLease, HostDeliveryLeaseId, HostDeliveryObservation, HostDeliveryRecord,
    HostDeliveryTarget, IntegratorFence, ProcessedActionRef,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::delivery::StoredHostDelivery;

use super::ceremony_store::{decode, encode, sqlx_error};
use super::PostgresPool;

/// Work handed out to hosts, shared by every replica of the service.
#[derive(Debug, Clone)]
pub struct PostgresHostDeliveryLedger {
    pool: PostgresPool,
}

impl PostgresHostDeliveryLedger {
    #[must_use]
    pub const fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    fn inner(&self) -> &PgPool {
        self.pool.inner()
    }

    async fn begin(
        &self,
        operation: &'static str,
    ) -> Result<Transaction<'_, Postgres>, DomainError> {
        self.inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, operation))
    }
}

#[async_trait]
impl HostDeliveryLedgerPort for PostgresHostDeliveryLedger {
    async fn enqueue(&self, record: HostDeliveryRecord) -> Result<EnqueueOutcome, DomainError> {
        let mut transaction = self.begin("begin host delivery enqueue").await?;
        if let Some(existing) = locked(&mut transaction, record.id()).await? {
            return Ok(EnqueueOutcome::AlreadyQueued(existing.into_record()));
        }
        upsert(&mut transaction, &StoredHostDelivery::new(record.clone())).await?;
        commit(transaction, "commit host delivery enqueue").await?;
        Ok(EnqueueOutcome::Enqueued(record))
    }

    async fn lease(
        &self,
        filter: &HostDeliveryFilter,
        owner: &HostAgentIncarnation,
        now: OffsetDateTime,
        duration: DurationMs,
        limit: HostDeliveryPageLimit,
    ) -> Result<Vec<LeasedDelivery>, DomainError> {
        let mut transaction = self.begin("begin host delivery lease").await?;
        let candidates = scan(&mut transaction, filter.target().keys().as_deref()).await?;
        let mut leased = Vec::new();
        for stored in candidates {
            if leased.len() == limit.as_usize() {
                break;
            }
            if !stored.is_offerable_at(now) || !filter.admits(stored.record()) {
                continue;
            }
            let (next, lease) = stored.leased(lease_id()?, owner, now, duration);
            leased.push(LeasedDelivery::new(lease, next.record().clone()));
            upsert(&mut transaction, &next).await?;
        }
        commit(transaction, "commit host delivery lease").await?;
        Ok(leased)
    }

    async fn acknowledge(
        &self,
        lease: &HostDeliveryLease,
        observation: &HostDeliveryObservation,
        now: OffsetDateTime,
    ) -> Result<AckOutcome, DomainError> {
        let mut transaction = self.begin("begin host delivery acknowledgement").await?;
        let Some(stored) = locked(&mut transaction, lease.delivery_id()).await? else {
            return Ok(AckOutcome::LeaseNotOwned);
        };
        let (next, outcome) = stored.acknowledged(lease, observation, now);
        if let Some(next) = next {
            upsert(&mut transaction, &next).await?;
            commit(transaction, "commit host delivery acknowledgement").await?;
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
        let mut transaction = self.begin("begin host delivery close").await?;
        let stored = locked(&mut transaction, delivery_id)
            .await?
            .ok_or(DomainError::NotFound {
                what: "host_delivery",
            })?;
        let (next, outcome) = stored.processed(owner, fence, action, now);
        if let Some(next) = next {
            upsert(&mut transaction, &next).await?;
            commit(transaction, "commit host delivery close").await?;
        }
        Ok(outcome)
    }

    async fn mark_failed(
        &self,
        lease: &HostDeliveryLease,
        reason: &DeliveryFailureReason,
        now: OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, DomainError> {
        let mut transaction = self.begin("begin host delivery failure").await?;
        let Some(stored) = locked(&mut transaction, lease.delivery_id()).await? else {
            return Ok(DeliveryFailureOutcome::LeaseNotOwned);
        };
        let (next, outcome) = stored.failed(lease, reason, now);
        if let Some(next) = next {
            upsert(&mut transaction, &next).await?;
            commit(transaction, "commit host delivery failure").await?;
        }
        Ok(outcome)
    }

    async fn release(
        &self,
        lease: &HostDeliveryLease,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut transaction = self.begin("begin host delivery release").await?;
        let released = locked(&mut transaction, lease.delivery_id())
            .await?
            .and_then(|stored| stored.released(lease, now));
        if let Some(next) = released {
            upsert(&mut transaction, &next).await?;
            commit(transaction, "commit host delivery release").await?;
        }
        Ok(())
    }

    async fn expire(&self, now: OffsetDateTime) -> Result<Vec<HostDeliveryId>, DomainError> {
        let mut transaction = self.begin("begin host delivery expiry").await?;
        let mut expired = Vec::new();
        for stored in scan(&mut transaction, None).await? {
            let Some(next) = stored.expired(now) else {
                continue;
            };
            expired.push(next.id().clone());
            upsert(&mut transaction, &next).await?;
        }
        commit(transaction, "commit host delivery expiry").await?;
        Ok(expired)
    }

    async fn supersede(
        &self,
        previous: &HostDeliveryTarget,
        replacement: &HostDeliveryTarget,
        follow: FollowReplacement,
        now: OffsetDateTime,
    ) -> Result<SupersessionOutcome, DomainError> {
        let mut transaction = self.begin("begin host delivery supersession").await?;
        let key = previous.target_key();
        let affected = scan(&mut transaction, Some(&[key])).await?;
        let mut superseded = Vec::new();
        let mut replacements = Vec::new();
        for stored in affected {
            if stored.record().target() != previous || stored.record().state().is_terminal() {
                continue;
            }
            let opened = if follow.follows() {
                let candidate = stored.re_addressed(replacement.clone(), now)?;
                let id = candidate.id().clone();
                if locked(&mut transaction, &id).await?.is_none() {
                    replacements.push(candidate.record().clone());
                    upsert(&mut transaction, &candidate).await?;
                }
                Some(id)
            } else {
                None
            };
            superseded.push(stored.id().clone());
            upsert(&mut transaction, &stored.superseded(opened, now)).await?;
        }
        commit(transaction, "commit host delivery supersession").await?;
        Ok(SupersessionOutcome::new(superseded, replacements))
    }

    async fn get(&self, id: &HostDeliveryId) -> Result<Option<HostDeliveryRecord>, DomainError> {
        let row = sqlx::query("SELECT payload FROM host_deliveries WHERE delivery_id = $1")
            .bind(id.as_str())
            .fetch_optional(self.inner())
            .await
            .map_err(|error| sqlx_error(error, "read host delivery"))?;
        row.map(|row| payload(&row).map(StoredHostDelivery::into_record))
            .transpose()
    }

    async fn list(&self, query: &HostDeliveryQuery) -> Result<HostDeliveryPage, DomainError> {
        // One more than asked for: the extra row is how the page knows
        // whether to offer a cursor, and it never reaches the caller.
        let wanted = query.limit().as_usize() + 1;
        let rows = sqlx::query(
            "SELECT payload FROM host_deliveries \
             WHERE ($1::text IS NULL OR ceremony_id = $1) \
               AND ($2::text IS NULL OR state = $2) \
               AND ($3::text IS NULL OR delivery_id > $3) \
             ORDER BY delivery_id",
        )
        .bind(query.ceremony_id().map(ToString::to_string))
        .bind(query.state().map(|state| state.as_str().to_owned()))
        .bind(query.cursor().map(ToString::to_string))
        .fetch_all(self.inner())
        .await
        .map_err(|error| sqlx_error(error, "list host deliveries"))?;

        let mut found = Vec::with_capacity(wanted);
        for row in rows {
            let stored: StoredHostDelivery = payload(&row)?;
            if query.admits(stored.record()) {
                found.push(stored.into_record());
                if found.len() == wanted {
                    break;
                }
            }
        }
        let more = found.len() == wanted;
        found.truncate(query.limit().as_usize());
        let next_cursor = more
            .then(|| found.last().map(|record| record.id().clone()))
            .flatten();
        Ok(HostDeliveryPage::new(found, next_cursor))
    }
}

/// One delivery, locked for the rest of this transaction.
async fn locked(
    transaction: &mut Transaction<'_, Postgres>,
    id: &HostDeliveryId,
) -> Result<Option<StoredHostDelivery>, DomainError> {
    let row = sqlx::query("SELECT payload FROM host_deliveries WHERE delivery_id = $1 FOR UPDATE")
        .bind(id.as_str())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|error| sqlx_error(error, "lock host delivery"))?;
    row.map(|row| payload(&row)).transpose()
}

/// Every delivery, or every delivery of some destinations, locked.
async fn scan(
    transaction: &mut Transaction<'_, Postgres>,
    target_keys: Option<&[made_core::value_objects::HostDeliveryTargetKey]>,
) -> Result<Vec<StoredHostDelivery>, DomainError> {
    let keys: Option<Vec<String>> =
        target_keys.map(|keys| keys.iter().map(ToString::to_string).collect());
    let rows = sqlx::query(
        "SELECT payload FROM host_deliveries \
         WHERE ($1::text[] IS NULL OR target_key = ANY($1)) \
         ORDER BY delivery_id FOR UPDATE",
    )
    .bind(keys)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "scan host deliveries"))?;
    rows.iter().map(payload).collect()
}

async fn upsert(
    transaction: &mut Transaction<'_, Postgres>,
    stored: &StoredHostDelivery,
) -> Result<(), DomainError> {
    sqlx::query(
        "INSERT INTO host_deliveries (delivery_id, ceremony_id, target_key, state, payload) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (delivery_id) DO UPDATE \
         SET state = EXCLUDED.state, target_key = EXCLUDED.target_key, \
             payload = EXCLUDED.payload",
    )
    .bind(stored.id().as_str())
    .bind(stored.record().item().ceremony_id().as_str())
    .bind(stored.target_key())
    .bind(stored.record().state().kind().as_str())
    .bind(encode(stored, "encode host delivery")?)
    .execute(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "write host delivery"))?;
    Ok(())
}

async fn commit(
    transaction: Transaction<'_, Postgres>,
    operation: &'static str,
) -> Result<(), DomainError> {
    transaction
        .commit()
        .await
        .map_err(|error| sqlx_error(error, operation))
}

fn payload(row: &sqlx::postgres::PgRow) -> Result<StoredHostDelivery, DomainError> {
    let bytes: Vec<u8> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "read host delivery payload"))?;
    decode(&bytes, "decode host delivery")
}

fn lease_id() -> Result<HostDeliveryLeaseId, DomainError> {
    HostDeliveryLeaseId::new(Uuid::new_v4().to_string())
}
