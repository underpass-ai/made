use async_trait::async_trait;
use made_core::entities::{AuditFact, AuditRecord};
use made_core::error::DomainError;
use made_core::ports::{
    seal_continuation, AppendOutcome, CeremonyEventStorePort, PositionedRecord,
};
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, GlobalPosition, StreamVersion};
use sqlx::{Postgres, Row, Transaction};

use super::ceremony_store::{decode, encode, i64_to_u64, sqlx_error, u64_to_i64};
use super::PostgresCeremonyStore;

async fn records_in(
    transaction: &mut Transaction<'_, Postgres>,
    stream: &CeremonyId,
) -> Result<Vec<AuditRecord>, DomainError> {
    let rows = sqlx::query(
        "SELECT sequence, payload FROM ceremony_events \
         WHERE stream_id = $1 ORDER BY sequence",
    )
    .bind(stream.as_str())
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "read ceremony stream for append"))?;
    rows.into_iter()
        .map(|row| decode_record(&row, Some(stream)))
        .collect()
}

fn decode_record(
    row: &sqlx::postgres::PgRow,
    expected_stream: Option<&CeremonyId>,
) -> Result<AuditRecord, DomainError> {
    let sequence: i64 = row
        .try_get("sequence")
        .map_err(|error| sqlx_error(error, "decode ceremony event sequence"))?;
    let bytes: Vec<u8> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "decode ceremony event payload"))?;
    let record: AuditRecord = decode(&bytes, "decode ceremony event")?;
    if expected_stream.is_some_and(|stream| record.ceremony_id() != stream)
        || record.sequence().value() != i64_to_u64(sequence)?
    {
        return Err(DomainError::InvariantViolated {
            reason: "postgres: ceremony event key does not match its payload",
        });
    }
    Ok(record)
}

async fn persist_records(
    transaction: &mut Transaction<'_, Postgres>,
    stream: &CeremonyId,
    records: &[AuditRecord],
) -> Result<GlobalPosition, DomainError> {
    let position_row = sqlx::query(
        "SELECT last_position FROM ceremony_store_global_position \
         WHERE singleton = TRUE FOR UPDATE",
    )
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "lock ceremony global position"))?;
    let last_position = i64_to_u64(
        position_row
            .try_get("last_position")
            .map_err(|error| sqlx_error(error, "decode ceremony global position"))?,
    )?;
    let first_position = GlobalPosition::new(last_position.saturating_add(1))?;
    let event_ids = records
        .iter()
        .map(|record| record.event_id().as_str())
        .collect::<Vec<_>>();
    let duplicate = sqlx::query(
        "SELECT 1 FROM ceremony_events \
         WHERE stream_id = $1 AND event_id = ANY($2) LIMIT 1",
    )
    .bind(stream.as_str())
    .bind(&event_ids)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "check ceremony event identity"))?;
    if duplicate.is_some() {
        return Err(DomainError::AlreadyExists {
            what: "ceremony_event",
        });
    }
    let mut position = first_position;
    for record in records {
        sqlx::query(
            "INSERT INTO ceremony_events \
             (stream_id, sequence, global_position, event_id, payload) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(stream.as_str())
        .bind(u64_to_i64(record.sequence().value())?)
        .bind(u64_to_i64(position.value())?)
        .bind(record.event_id().as_str())
        .bind(encode(record, "encode ceremony event")?)
        .execute(&mut **transaction)
        .await
        .map_err(|error| sqlx_error(error, "insert ceremony event"))?;
        position = position.next();
    }
    sqlx::query(
        "UPDATE ceremony_store_global_position SET last_position = $1 \
         WHERE singleton = TRUE",
    )
    .bind(u64_to_i64(position.value().saturating_sub(1))?)
    .execute(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "advance ceremony global position"))?;
    Ok(first_position)
}

#[async_trait]
impl CeremonyEventStorePort for PostgresCeremonyStore {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin ceremony append"))?;
        sqlx::query(
            "INSERT INTO ceremony_streams (stream_id, version) VALUES ($1, 0) \
             ON CONFLICT (stream_id) DO NOTHING",
        )
        .bind(stream.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "create ceremony stream"))?;
        let row =
            sqlx::query("SELECT version FROM ceremony_streams WHERE stream_id = $1 FOR UPDATE")
                .bind(stream.as_str())
                .fetch_one(&mut *transaction)
                .await
                .map_err(|error| sqlx_error(error, "lock ceremony stream"))?;
        let actual = StreamVersion::new(i64_to_u64(
            row.try_get("version")
                .map_err(|error| sqlx_error(error, "decode ceremony stream version"))?,
        )?);
        if actual != expected {
            return Ok(AppendOutcome::Conflict { expected, actual });
        }
        let existing = records_in(&mut transaction, stream).await?;
        let sealed = seal_continuation(stream, &existing, facts)?;

        let first_position = persist_records(&mut transaction, stream, &sealed).await?;
        let version = sealed.last().map_or(actual, |record| {
            StreamVersion::from_sequence(record.sequence())
        });
        sqlx::query("UPDATE ceremony_streams SET version = $2 WHERE stream_id = $1")
            .bind(stream.as_str())
            .bind(u64_to_i64(version.value())?)
            .execute(&mut *transaction)
            .await
            .map_err(|error| sqlx_error(error, "advance ceremony stream"))?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit ceremony append"))?;

        Ok(AppendOutcome::Appended {
            version,
            records: sealed,
            first_position,
        })
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        let rows = sqlx::query(
            "SELECT sequence, payload FROM ceremony_events \
             WHERE stream_id = $1 AND sequence > $2 ORDER BY sequence LIMIT $3",
        )
        .bind(stream.as_str())
        .bind(u64_to_i64(after.value())?)
        .bind(i64::try_from(limit.value()).unwrap_or(i64::MAX))
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "read ceremony events"))?;
        rows.into_iter()
            .map(|row| decode_record(&row, Some(stream)))
            .collect()
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        let rows = sqlx::query(
            "SELECT sequence, global_position, payload FROM ceremony_events \
             WHERE global_position >= $1 ORDER BY global_position LIMIT $2",
        )
        .bind(u64_to_i64(from.value())?)
        .bind(i64::try_from(limit.value()).unwrap_or(i64::MAX))
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "read global ceremony events"))?;
        rows.into_iter()
            .map(|row| {
                let position: i64 = row
                    .try_get("global_position")
                    .map_err(|error| sqlx_error(error, "decode ceremony event global position"))?;
                Ok(PositionedRecord {
                    position: GlobalPosition::new(i64_to_u64(position)?)?,
                    record: decode_record(&row, None)?,
                })
            })
            .collect()
    }

    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        let row = sqlx::query("SELECT version FROM ceremony_streams WHERE stream_id = $1")
            .bind(stream.as_str())
            .fetch_optional(self.pool.inner())
            .await
            .map_err(|error| sqlx_error(error, "read ceremony stream head"))?;
        row.map_or(Ok(StreamVersion::EMPTY), |row| {
            let version: i64 = row
                .try_get("version")
                .map_err(|error| sqlx_error(error, "decode ceremony stream head"))?;
            Ok(StreamVersion::new(i64_to_u64(version)?))
        })
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        let rows = sqlx::query("SELECT stream_id FROM ceremony_streams ORDER BY stream_id")
            .fetch_all(self.pool.inner())
            .await
            .map_err(|error| sqlx_error(error, "list ceremony streams"))?;
        rows.into_iter()
            .map(|row| {
                let stream: String = row
                    .try_get("stream_id")
                    .map_err(|error| sqlx_error(error, "decode ceremony stream id"))?;
                CeremonyId::new(stream)
            })
            .collect()
    }
}
