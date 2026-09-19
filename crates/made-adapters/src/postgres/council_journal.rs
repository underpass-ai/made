use super::council_journal_store::{append, begin, load_cursor, ordinal, write_cursor};
use super::error::{serde_to_domain, sqlx_to_domain};
use super::PostgresPool;
use async_trait::async_trait;
use made_core::entities::{CouncilJournalEvent, CouncilJournalRecord};
use made_core::error::DomainError;
use made_core::ports::CouncilJournalPort;
use made_core::value_objects::{
    AuthorizationEvidence, CouncilJournalConsumer, CouncilJournalLease, CouncilJournalLeaseId,
    CouncilJournalPageLimit, CouncilJournalPosition, DurationMs,
};
use serde_json::Value;
use sqlx::Row;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct PostgresCouncilJournal {
    pool: PostgresPool,
}
impl PostgresCouncilJournal {
    #[must_use]
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }
}
#[async_trait]
impl CouncilJournalPort for PostgresCouncilJournal {
    async fn publish(
        &self,
        event: CouncilJournalEvent,
    ) -> Result<CouncilJournalRecord, DomainError> {
        self.publish_authorized(event, None).await
    }
    async fn publish_authorized(
        &self,
        event: CouncilJournalEvent,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<CouncilJournalRecord, DomainError> {
        if event.publication_id().is_none() {
            return Err(DomainError::InvariantViolated {
                reason: "council publication requires an original event id",
            });
        }
        let mut tx = begin(&self.pool).await?;
        let record = append(&mut tx, event, authorization).await?;
        tx.commit()
            .await
            .map_err(|e| sqlx_to_domain(e, "commit council publication"))?;
        Ok(record)
    }
    async fn read(
        &self,
        after: Option<CouncilJournalPosition>,
        limit: CouncilJournalPageLimit,
    ) -> Result<Vec<CouncilJournalRecord>, DomainError> {
        let after = after.map(ordinal).transpose()?.unwrap_or(0);
        sqlx::query("SELECT position, record FROM council_journal WHERE position > $1 ORDER BY position LIMIT $2")
            .bind(after).bind(limit.value() as i64).fetch_all(self.pool.inner()).await
            .map_err(|e| sqlx_to_domain(e, "read council journal"))?.into_iter().map(|row| {
                let position: i64 = row.try_get("position").map_err(|e| sqlx_to_domain(e, "read council ordinal"))?;
                let body: Value = row.try_get("record").map_err(|e| sqlx_to_domain(e, "read council record"))?;
                let record: CouncilJournalRecord = serde_json::from_value(body).map_err(|e| serde_to_domain(&e, "decode council record"))?;
                if ordinal(record.position())? != position {
                    return Err(DomainError::InvariantViolated { reason: "council journal key differs from record position" });
                }
                Ok(record)
            }).collect()
    }
    async fn position(
        &self,
        consumer: &CouncilJournalConsumer,
    ) -> Result<Option<CouncilJournalPosition>, DomainError> {
        let mut tx = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|e| sqlx_to_domain(e, "read council cursor"))?;
        Ok(load_cursor(&mut tx, consumer).await?.position)
    }
    async fn lease(
        &self,
        consumer: &CouncilJournalConsumer,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CouncilJournalLease>, DomainError> {
        if duration.get() == 0 || duration.get() > 3_600_000 {
            return Err(DomainError::InvariantViolated {
                reason: "council cursor lease must be between one millisecond and one hour",
            });
        }
        let expires_at = now
            .checked_add(time::Duration::milliseconds(duration.get() as i64))
            .ok_or(DomainError::InvariantViolated {
                reason: "council cursor lease deadline overflow",
            })?;
        let mut tx = begin(&self.pool).await?;
        let mut stored = load_cursor(&mut tx, consumer).await?;
        if stored
            .lease
            .as_ref()
            .is_some_and(|lease| lease.expires_at() > now)
        {
            return Ok(None);
        }
        let lease = CouncilJournalLease::new(
            consumer.clone(),
            CouncilJournalLeaseId::new(uuid::Uuid::new_v4().to_string())?,
            stored.position,
            expires_at,
        );
        stored.lease = Some(lease.clone());
        write_cursor(&mut tx, consumer, &stored).await?;
        tx.commit()
            .await
            .map_err(|e| sqlx_to_domain(e, "commit council lease"))?;
        Ok(Some(lease))
    }
    async fn acknowledge(
        &self,
        lease: &CouncilJournalLease,
        through: CouncilJournalPosition,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut tx = begin(&self.pool).await?;
        let mut stored = load_cursor(&mut tx, lease.consumer()).await?;
        if stored.lease.as_ref() != Some(lease) || lease.expires_at() <= now {
            return Err(DomainError::InvariantViolated {
                reason: "council consumer lease is expired or no longer owned",
            });
        }
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM council_journal WHERE position = $1)")
                .bind(ordinal(through)?)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| sqlx_to_domain(e, "validate council acknowledgement"))?;
        if !exists || stored.position.is_some_and(|position| through < position) {
            return Err(DomainError::InvariantViolated {
                reason: "council acknowledgement must advance to an existing record",
            });
        }
        stored.position = Some(through);
        stored.lease = None;
        write_cursor(&mut tx, lease.consumer(), &stored).await?;
        tx.commit()
            .await
            .map_err(|e| sqlx_to_domain(e, "commit council acknowledgement"))
    }
    async fn release(
        &self,
        lease: &CouncilJournalLease,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut tx = begin(&self.pool).await?;
        let mut stored = load_cursor(&mut tx, lease.consumer()).await?;
        if stored.lease.as_ref() != Some(lease) || lease.expires_at() <= now {
            return Err(DomainError::InvariantViolated {
                reason: "council consumer lease is expired or no longer owned",
            });
        }
        stored.lease = None;
        write_cursor(&mut tx, lease.consumer(), &stored).await?;
        tx.commit()
            .await
            .map_err(|e| sqlx_to_domain(e, "release council lease"))
    }
}
