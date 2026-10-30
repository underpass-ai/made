use std::time::Duration;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::ProtocolEventCursorPort;
use made_core::value_objects::{
    DurationMs, GlobalPosition, ProtocolEventConsumer, ProtocolEventCursorAttempt,
    ProtocolEventCursorLease, ProtocolEventCursorLeaseId, ProtocolEventQuarantineReason,
    QuarantinedProtocolEvent,
};
use sqlx::{Postgres, Row, Transaction};
use time::OffsetDateTime;

use super::postgres_stored_cursor::PostgresStoredCursor;
use super::protocol_store::{decode, encode, i64_to_u64, sqlx_error, u64_to_i64};
use super::PostgresProtocolStore;

async fn lock_cursor(
    transaction: &mut Transaction<'_, Postgres>,
    consumer: &ProtocolEventConsumer,
) -> Result<PostgresStoredCursor, DomainError> {
    sqlx::query(
        "INSERT INTO protocol_event_cursors (consumer, payload) VALUES ($1, NULL) \
         ON CONFLICT (consumer) DO NOTHING",
    )
    .bind(consumer.as_str())
    .execute(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "create protocol event cursor"))?;
    let row =
        sqlx::query("SELECT payload FROM protocol_event_cursors WHERE consumer = $1 FOR UPDATE")
            .bind(consumer.as_str())
            .fetch_one(&mut **transaction)
            .await
            .map_err(|error| sqlx_error(error, "lock protocol event cursor"))?;
    let payload: Option<Vec<u8>> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "decode protocol event cursor payload"))?;
    payload.map_or(Ok(PostgresStoredCursor::default()), |payload| {
        decode(&payload, "decode protocol event cursor")
    })
}

async fn write_cursor(
    transaction: &mut Transaction<'_, Postgres>,
    consumer: &ProtocolEventConsumer,
    cursor: &PostgresStoredCursor,
) -> Result<(), DomainError> {
    sqlx::query("UPDATE protocol_event_cursors SET payload = $2 WHERE consumer = $1")
        .bind(consumer.as_str())
        .bind(encode(cursor, "encode protocol event cursor")?)
        .execute(&mut **transaction)
        .await
        .map_err(|error| sqlx_error(error, "write protocol event cursor"))?;
    Ok(())
}

fn require_lease(
    stored: &PostgresStoredCursor,
    lease: &ProtocolEventCursorLease,
    position: GlobalPosition,
) -> Result<(), DomainError> {
    let matches = stored
        .lease
        .as_ref()
        .is_some_and(|active| active.lease_id() == lease.lease_id());
    if !matches || position != lease.next_position() {
        return Err(DomainError::Conflict {
            what: "protocol_event_cursor",
        });
    }
    Ok(())
}

#[async_trait]
impl ProtocolEventCursorPort for PostgresProtocolStore {
    async fn position(
        &self,
        consumer: &ProtocolEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError> {
        let row = sqlx::query("SELECT payload FROM protocol_event_cursors WHERE consumer = $1")
            .bind(consumer.as_str())
            .fetch_optional(self.pool.inner())
            .await
            .map_err(|error| sqlx_error(error, "read protocol event cursor"))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let payload: Option<Vec<u8>> = row
            .try_get("payload")
            .map_err(|error| sqlx_error(error, "decode protocol event cursor payload"))?;
        payload.map_or(Ok(None), |payload| {
            decode::<PostgresStoredCursor>(&payload, "decode protocol event cursor")
                .map(|cursor| cursor.acknowledged_through)
        })
    }

    async fn lease(
        &self,
        consumer: &ProtocolEventConsumer,
        lease_id: ProtocolEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<ProtocolEventCursorLease>, DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin protocol cursor lease"))?;
        let mut stored = lock_cursor(&mut transaction, consumer).await?;
        if stored
            .lease
            .as_ref()
            .is_some_and(|active| active.leased_until() > now)
        {
            return Ok(None);
        }
        let lease = ProtocolEventCursorLease::new(
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
            .map_err(|error| sqlx_error(error, "commit protocol cursor lease"))?;
        Ok(Some(lease))
    }

    async fn acknowledge(
        &self,
        consumer: &ProtocolEventConsumer,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin protocol cursor acknowledge"))?;
        let mut stored = lock_cursor(&mut transaction, consumer).await?;
        if stored
            .acknowledged_through
            .is_some_and(|current| through <= current)
        {
            return Ok(());
        }
        if stored.lease.is_some() {
            return Err(DomainError::Conflict {
                what: "protocol_event_cursor",
            });
        }
        stored.acknowledged_through = Some(through);
        stored.attempt = ProtocolEventCursorAttempt::NONE;
        write_cursor(&mut transaction, consumer, &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit protocol cursor acknowledge"))?;
        Ok(())
    }

    async fn acknowledge_lease(
        &self,
        lease: &ProtocolEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin leased protocol acknowledge"))?;
        let mut stored = lock_cursor(&mut transaction, lease.consumer()).await?;
        require_lease(&stored, lease, through)?;
        stored.acknowledged_through = Some(through);
        stored.attempt = ProtocolEventCursorAttempt::NONE;
        stored.lease = None;
        write_cursor(&mut transaction, lease.consumer(), &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit leased protocol acknowledge"))?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        lease: &ProtocolEventCursorLease,
        position: GlobalPosition,
    ) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin protocol cursor failure"))?;
        let mut stored = lock_cursor(&mut transaction, lease.consumer()).await?;
        require_lease(&stored, lease, position)?;
        stored.attempt = stored.attempt.next();
        stored.lease = None;
        write_cursor(&mut transaction, lease.consumer(), &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit protocol cursor failure"))?;
        Ok(())
    }

    async fn quarantine(
        &self,
        lease: &ProtocolEventCursorLease,
        position: GlobalPosition,
        reason: ProtocolEventQuarantineReason,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin protocol quarantine"))?;
        let mut stored = lock_cursor(&mut transaction, lease.consumer()).await?;
        require_lease(&stored, lease, position)?;
        let quarantined = QuarantinedProtocolEvent::new(
            lease.consumer().clone(),
            position,
            stored.attempt,
            reason,
            now,
        );
        sqlx::query(
            "INSERT INTO protocol_event_quarantine (consumer, global_position, payload) \
             VALUES ($1, $2, $3)",
        )
        .bind(lease.consumer().as_str())
        .bind(u64_to_i64(position.value())?)
        .bind(encode(&quarantined, "encode quarantined protocol event")?)
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "insert quarantined protocol event"))?;
        stored.acknowledged_through = Some(position);
        stored.attempt = ProtocolEventCursorAttempt::NONE;
        stored.lease = None;
        write_cursor(&mut transaction, lease.consumer(), &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit protocol quarantine"))?;
        Ok(())
    }

    async fn release(&self, lease: &ProtocolEventCursorLease) -> Result<(), DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin protocol cursor release"))?;
        let mut stored = lock_cursor(&mut transaction, lease.consumer()).await?;
        if !stored
            .lease
            .as_ref()
            .is_some_and(|active| active.lease_id() == lease.lease_id())
        {
            return Err(DomainError::Conflict {
                what: "protocol_event_cursor",
            });
        }
        stored.lease = None;
        write_cursor(&mut transaction, lease.consumer(), &stored).await?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit protocol cursor release"))?;
        Ok(())
    }

    async fn quarantined(
        &self,
        consumer: &ProtocolEventConsumer,
    ) -> Result<Vec<QuarantinedProtocolEvent>, DomainError> {
        let rows = sqlx::query(
            "SELECT global_position, payload FROM protocol_event_quarantine \
             WHERE consumer = $1 ORDER BY global_position",
        )
        .bind(consumer.as_str())
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "read quarantined protocol events"))?;
        rows.into_iter()
            .map(|row| {
                let position: i64 = row
                    .try_get("global_position")
                    .map_err(|error| sqlx_error(error, "decode quarantined protocol position"))?;
                let payload: Vec<u8> = row
                    .try_get("payload")
                    .map_err(|error| sqlx_error(error, "decode quarantined protocol payload"))?;
                let quarantined: QuarantinedProtocolEvent =
                    decode(&payload, "decode quarantined protocol event")?;
                if quarantined.consumer() != consumer
                    || quarantined.position().value() != i64_to_u64(position)?
                {
                    return Err(DomainError::InvariantViolated {
                        reason: "postgres: quarantined protocol key does not match its payload",
                    });
                }
                Ok(quarantined)
            })
            .collect()
    }
}
