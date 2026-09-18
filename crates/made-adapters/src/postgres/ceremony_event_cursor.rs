use std::time::Duration;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::CeremonyEventCursorPort;
use made_core::value_objects::{
    CeremonyEventConsumer, CeremonyEventCursorAttempt, CeremonyEventCursorLease,
    CeremonyEventCursorLeaseId, CeremonyEventQuarantineReason, DurationMs, GlobalPosition,
    QuarantinedCeremonyEvent,
};
use sqlx::{Postgres, Row, Transaction};
use time::OffsetDateTime;

use super::ceremony_store::{decode, encode, i64_to_u64, sqlx_error, u64_to_i64};
use super::postgres_stored_cursor::PostgresStoredCursor;
use super::PostgresCeremonyStore;

async fn lock_cursor(
    transaction: &mut Transaction<'_, Postgres>,
    consumer: &CeremonyEventConsumer,
) -> Result<PostgresStoredCursor, DomainError> {
    sqlx::query(
        "INSERT INTO ceremony_event_cursors (consumer, payload) VALUES ($1, NULL) \
         ON CONFLICT (consumer) DO NOTHING",
    )
    .bind(consumer.as_str())
    .execute(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "create ceremony event cursor"))?;
    let row =
        sqlx::query("SELECT payload FROM ceremony_event_cursors WHERE consumer = $1 FOR UPDATE")
            .bind(consumer.as_str())
            .fetch_one(&mut **transaction)
            .await
            .map_err(|error| sqlx_error(error, "lock ceremony event cursor"))?;
    let payload: Option<Vec<u8>> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "decode ceremony event cursor payload"))?;
    payload.map_or(Ok(PostgresStoredCursor::default()), |payload| {
        decode(&payload, "decode ceremony event cursor")
    })
}

async fn write_cursor(
    transaction: &mut Transaction<'_, Postgres>,
    consumer: &CeremonyEventConsumer,
    cursor: &PostgresStoredCursor,
) -> Result<(), DomainError> {
    sqlx::query("UPDATE ceremony_event_cursors SET payload = $2 WHERE consumer = $1")
        .bind(consumer.as_str())
        .bind(encode(cursor, "encode ceremony event cursor")?)
        .execute(&mut **transaction)
        .await
        .map_err(|error| sqlx_error(error, "write ceremony event cursor"))?;
    Ok(())
}

fn require_lease(
    stored: &PostgresStoredCursor,
    lease: &CeremonyEventCursorLease,
    position: GlobalPosition,
) -> Result<(), DomainError> {
    let matches = stored
        .lease
        .as_ref()
        .is_some_and(|active| active.lease_id() == lease.lease_id());
    if !matches || position != lease.next_position() {
        return Err(DomainError::Conflict {
            what: "ceremony_event_cursor",
        });
    }
    Ok(())
}

#[async_trait]
impl CeremonyEventCursorPort for PostgresCeremonyStore {
    async fn position(
        &self,
        consumer: &CeremonyEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError> {
        let row = sqlx::query("SELECT payload FROM ceremony_event_cursors WHERE consumer = $1")
            .bind(consumer.as_str())
            .fetch_optional(self.pool.inner())
            .await
            .map_err(|error| sqlx_error(error, "read ceremony event cursor"))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let payload: Option<Vec<u8>> = row
            .try_get("payload")
            .map_err(|error| sqlx_error(error, "decode ceremony event cursor payload"))?;
        payload.map_or(Ok(None), |payload| {
            decode::<PostgresStoredCursor>(&payload, "decode ceremony event cursor")
                .map(|cursor| cursor.acknowledged_through)
        })
    }

    async fn lease(
        &self,
        consumer: &CeremonyEventConsumer,
        lease_id: CeremonyEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CeremonyEventCursorLease>, DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin ceremony cursor lease"))?;
        let mut stored = lock_cursor(&mut transaction, consumer).await?;
        if stored
            .lease
            .as_ref()
            .is_some_and(|active| active.leased_until() > now)
        {
            return Ok(None);
        }
        let lease = CeremonyEventCursorLease::new(
            consumer.clone(),
            lease_id,
            stored.acknowledged_through,
            stored.attempt,
            now + Duration::from_millis(duration.get()),
        );
        stored.lease = Some(lease.clone());
        write_cursor(&mut transaction, consumer, &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit ceremony cursor lease"))?;
        Ok(Some(lease))
    }

    async fn acknowledge(
        &self,
        consumer: &CeremonyEventConsumer,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin ceremony cursor acknowledge"))?;
        let mut stored = lock_cursor(&mut transaction, consumer).await?;
        if stored
            .acknowledged_through
            .is_some_and(|current| through <= current)
        {
            return Ok(());
        }
        if stored.lease.is_some() {
            return Err(DomainError::Conflict {
                what: "ceremony_event_cursor",
            });
        }
        stored.acknowledged_through = Some(through);
        stored.attempt = CeremonyEventCursorAttempt::NONE;
        write_cursor(&mut transaction, consumer, &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit ceremony cursor acknowledge"))?;
        Ok(())
    }

    async fn acknowledge_lease(
        &self,
        lease: &CeremonyEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin leased ceremony acknowledge"))?;
        let mut stored = lock_cursor(&mut transaction, lease.consumer()).await?;
        require_lease(&stored, lease, through)?;
        stored.acknowledged_through = Some(through);
        stored.attempt = CeremonyEventCursorAttempt::NONE;
        stored.lease = None;
        write_cursor(&mut transaction, lease.consumer(), &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit leased ceremony acknowledge"))?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
    ) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin ceremony cursor failure"))?;
        let mut stored = lock_cursor(&mut transaction, lease.consumer()).await?;
        require_lease(&stored, lease, position)?;
        stored.attempt = stored.attempt.next();
        stored.lease = None;
        write_cursor(&mut transaction, lease.consumer(), &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit ceremony cursor failure"))?;
        Ok(())
    }

    async fn quarantine(
        &self,
        lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
        reason: CeremonyEventQuarantineReason,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin ceremony quarantine"))?;
        let mut stored = lock_cursor(&mut transaction, lease.consumer()).await?;
        require_lease(&stored, lease, position)?;
        let quarantined = QuarantinedCeremonyEvent::new(
            lease.consumer().clone(),
            position,
            stored.attempt,
            reason,
            now,
        );
        sqlx::query(
            "INSERT INTO ceremony_event_quarantine (consumer, global_position, payload) \
             VALUES ($1, $2, $3)",
        )
        .bind(lease.consumer().as_str())
        .bind(u64_to_i64(position.value())?)
        .bind(encode(&quarantined, "encode quarantined ceremony event")?)
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "insert quarantined ceremony event"))?;
        stored.acknowledged_through = Some(position);
        stored.attempt = CeremonyEventCursorAttempt::NONE;
        stored.lease = None;
        write_cursor(&mut transaction, lease.consumer(), &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit ceremony quarantine"))?;
        Ok(())
    }

    async fn release(&self, lease: &CeremonyEventCursorLease) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin ceremony cursor release"))?;
        let mut stored = lock_cursor(&mut transaction, lease.consumer()).await?;
        if !stored
            .lease
            .as_ref()
            .is_some_and(|active| active.lease_id() == lease.lease_id())
        {
            return Err(DomainError::Conflict {
                what: "ceremony_event_cursor",
            });
        }
        stored.lease = None;
        write_cursor(&mut transaction, lease.consumer(), &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit ceremony cursor release"))?;
        Ok(())
    }

    async fn quarantined(
        &self,
        consumer: &CeremonyEventConsumer,
    ) -> Result<Vec<QuarantinedCeremonyEvent>, DomainError> {
        let rows = sqlx::query(
            "SELECT global_position, payload FROM ceremony_event_quarantine \
             WHERE consumer = $1 ORDER BY global_position",
        )
        .bind(consumer.as_str())
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "read quarantined ceremony events"))?;
        rows.into_iter()
            .map(|row| {
                let position: i64 = row
                    .try_get("global_position")
                    .map_err(|error| sqlx_error(error, "decode quarantined ceremony position"))?;
                let payload: Vec<u8> = row
                    .try_get("payload")
                    .map_err(|error| sqlx_error(error, "decode quarantined ceremony payload"))?;
                let quarantined: QuarantinedCeremonyEvent =
                    decode(&payload, "decode quarantined ceremony event")?;
                if quarantined.consumer() != consumer
                    || quarantined.position().value() != i64_to_u64(position)?
                {
                    return Err(DomainError::InvariantViolated {
                        reason: "postgres: quarantined ceremony key does not match its payload",
                    });
                }
                Ok(quarantined)
            })
            .collect()
    }
}
