use super::error::{serde_to_domain, sqlx_to_domain};
use super::PostgresPool;
use crate::stored_council_cursor::StoredCouncilCursor;
use made_core::entities::{CouncilJournalEvent, CouncilJournalRecord};
use made_core::error::DomainError;
use made_core::value_objects::{CouncilJournalConsumer, CouncilJournalPosition};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};

/// Every council writer locks this first, so journal order equals commit order
/// and registry, journal and counter changes are one transaction.
pub(super) async fn begin(pool: &PostgresPool) -> Result<Transaction<'_, Postgres>, DomainError> {
    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|e| sqlx_to_domain(e, "begin council write"))?;
    sqlx::query("SELECT last_position FROM council_journal_meta WHERE singleton = TRUE FOR UPDATE")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| sqlx_to_domain(e, "lock council order"))?;
    Ok(tx)
}

pub(super) async fn append(
    tx: &mut Transaction<'_, Postgres>,
    event: CouncilJournalEvent,
) -> Result<CouncilJournalRecord, DomainError> {
    if let Some(id) = event.publication_id() {
        let existing: Option<Value> =
            sqlx::query_scalar("SELECT record FROM council_journal WHERE publication_id = $1")
                .bind(id.as_str())
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| sqlx_to_domain(e, "read council publication"))?;
        if let Some(existing) = existing {
            let record: CouncilJournalRecord = serde_json::from_value(existing)
                .map_err(|e| serde_to_domain(&e, "read council publication"))?;
            if record.event() != &event {
                return Err(DomainError::InvariantViolated {
                    reason: "council publication id already has a different fact",
                });
            }
            return Ok(record);
        }
    }
    let previous: i64 =
        sqlx::query_scalar("SELECT last_position FROM council_journal_meta WHERE singleton = TRUE")
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| sqlx_to_domain(e, "read council order"))?;
    let next = previous
        .checked_add(1)
        .ok_or(DomainError::InvariantViolated {
            reason: "council journal position exhausted",
        })?;
    let position = CouncilJournalPosition::new(u64::try_from(next).map_err(|_| {
        DomainError::InvariantViolated {
            reason: "council journal position is corrupt",
        }
    })?)?;
    let record = CouncilJournalRecord::new(position, event);
    let encoded =
        serde_json::to_value(&record).map_err(|e| serde_to_domain(&e, "encode council record"))?;
    sqlx::query(
        "INSERT INTO council_journal (position, publication_id, record) VALUES ($1, $2, $3)",
    )
    .bind(next)
    .bind(
        record
            .event()
            .publication_id()
            .map(made_core::value_objects::EventId::as_str),
    )
    .bind(encoded)
    .execute(&mut **tx)
    .await
    .map_err(|e| sqlx_to_domain(e, "append council record"))?;
    sqlx::query("UPDATE council_journal_meta SET last_position = $1 WHERE singleton = TRUE")
        .bind(next)
        .execute(&mut **tx)
        .await
        .map_err(|e| sqlx_to_domain(e, "advance council order"))?;
    Ok(record)
}

pub(super) async fn load_cursor(
    tx: &mut Transaction<'_, Postgres>,
    consumer: &CouncilJournalConsumer,
) -> Result<StoredCouncilCursor, DomainError> {
    let row = sqlx::query("SELECT state FROM council_journal_cursors WHERE consumer = $1")
        .bind(consumer.as_str())
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| sqlx_to_domain(e, "read council cursor"))?;
    row.map(|row| {
        let value: Value = row
            .try_get("state")
            .map_err(|e| sqlx_to_domain(e, "read council cursor"))?;
        serde_json::from_value(value).map_err(|e| serde_to_domain(&e, "read council cursor"))
    })
    .transpose()
    .map(Option::unwrap_or_default)
}

pub(super) async fn write_cursor(
    tx: &mut Transaction<'_, Postgres>,
    consumer: &CouncilJournalConsumer,
    state: &StoredCouncilCursor,
) -> Result<(), DomainError> {
    let value =
        serde_json::to_value(state).map_err(|e| serde_to_domain(&e, "encode council cursor"))?;
    sqlx::query("INSERT INTO council_journal_cursors (consumer, state) VALUES ($1, $2) ON CONFLICT (consumer) DO UPDATE SET state = EXCLUDED.state")
        .bind(consumer.as_str()).bind(value).execute(&mut **tx).await.map_err(|e| sqlx_to_domain(e, "write council cursor"))?;
    Ok(())
}

pub(super) fn ordinal(value: CouncilJournalPosition) -> Result<i64, DomainError> {
    i64::try_from(value.value()).map_err(|_| DomainError::InvariantViolated {
        reason: "council journal position exceeds database range",
    })
}
