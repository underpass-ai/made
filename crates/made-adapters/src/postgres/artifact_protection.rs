use made_core::ports::{
    ArtifactIdempotencyKey, ArtifactRecord, ArtifactSnapshot, ArtifactStoreError,
};
use made_core::value_objects::ArtifactId;
use sqlx::{Postgres, Row, Transaction};

use super::artifact_blob_queries::hash_blob_chunks;
use super::artifact_store::{storage_failure, PostgresArtifactStore};

impl PostgresArtifactStore {
    const PROTECTION_LOCK: i64 = 4_778_648_291_043_521_119;

    pub(super) async fn database_identity(&self) -> Result<String, ArtifactStoreError> {
        let row = sqlx::query(
            "SELECT (pg_control_system()).system_identifier::text AS system_id, oid::text AS database_id FROM pg_database WHERE datname = current_database()",
        )
        .fetch_one(self.pool.inner())
        .await
        .map_err(|error| storage_failure(&error))?;
        Ok(format!(
            "{}:{}",
            row.try_get::<String, _>("system_id")
                .map_err(|error| storage_failure(&error))?,
            row.try_get::<String, _>("database_id")
                .map_err(|error| storage_failure(&error))?
        ))
    }

    pub(super) async fn lock_protection_barrier(
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ArtifactStoreError> {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(Self::PROTECTION_LOCK)
            .execute(&mut **tx)
            .await
            .map_err(|error| storage_failure(&error))?;
        Ok(())
    }

    pub(super) async fn existing_protection(
        tx: &mut Transaction<'_, Postgres>,
        key: &ArtifactIdempotencyKey,
    ) -> Result<Option<ArtifactSnapshot>, ArtifactStoreError> {
        let row = sqlx::query(
            "SELECT body FROM artifact_protections WHERE protection_key = $1 FOR UPDATE",
        )
        .bind(key.as_str())
        .fetch_optional(&mut **tx)
        .await
        .map_err(|error| storage_failure(&error))?;
        row.map(|row| {
            serde_json::from_value(
                row.try_get("body")
                    .map_err(|error| storage_failure(&error))?,
            )
            .map_err(|error| {
                ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
            })
        })
        .transpose()
    }

    pub(super) async fn persist_protection(
        tx: &mut Transaction<'_, Postgres>,
        snapshot: &ArtifactSnapshot,
    ) -> Result<(), ArtifactStoreError> {
        sqlx::query(
            "INSERT INTO artifact_protections (protection_key, body, state) VALUES ($1, $2, 'protected')",
        )
        .bind(snapshot.key.as_str())
        .bind(serde_json::to_value(snapshot).map_err(|error| ArtifactStoreError::unavailable("serialize or decode artifact metadata", error))?)
        .execute(&mut **tx)
        .await
        .map_err(|error| storage_failure(&error))?;
        Ok(())
    }

    async fn load_protected_records(
        tx: &mut Transaction<'_, Postgres>,
        ids: Option<&[ArtifactId]>,
    ) -> Result<Vec<ArtifactRecord>, ArtifactStoreError> {
        let rows = match ids {
            Some(ids) => {
                let values: Vec<&str> = ids.iter().map(ArtifactId::as_str).collect();
                sqlx::query("SELECT body FROM artifact_records WHERE artifact_id = ANY($1) ORDER BY artifact_id FOR SHARE")
                    .bind(&values).fetch_all(&mut **tx).await.map_err(|error| storage_failure(&error))?
            }
            None => sqlx::query("SELECT body FROM artifact_records ORDER BY artifact_id FOR SHARE")
                .fetch_all(&mut **tx)
                .await
                .map_err(|error| storage_failure(&error))?,
        };
        let records: Vec<ArtifactRecord> = rows
            .into_iter()
            .map(|row| {
                serde_json::from_value(
                    row.try_get("body")
                        .map_err(|error| storage_failure(&error))?,
                )
                .map_err(|error| {
                    ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
                })
            })
            .collect::<Result<_, _>>()?;
        if ids.is_some_and(|ids| records.len() != ids.len()) {
            return Err(ArtifactStoreError::NotFound);
        }
        Self::validate_protected_content(tx, &records).await?;
        Ok(records)
    }

    pub(super) async fn validate_protected_content(
        tx: &mut Transaction<'_, Postgres>,
        records: &[ArtifactRecord],
    ) -> Result<(), ArtifactStoreError> {
        for record in records {
            let (digest, size) = hash_blob_chunks(
                tx,
                record.artifact.digest().as_str(),
                record.artifact.size_bytes().get(),
            )
            .await?;
            let matches = digest == record.artifact.digest().as_str()
                && size == record.artifact.size_bytes().get();
            let retired_and_absent = record.tombstone.is_some() && size == 0;
            if !matches && !retired_and_absent {
                return Err(ArtifactStoreError::FinalDigestMismatch);
            }
        }
        Ok(())
    }

    pub(super) async fn protect_records(
        &self,
        key: ArtifactIdempotencyKey,
        ids: Option<Vec<ArtifactId>>,
    ) -> Result<ArtifactSnapshot, ArtifactStoreError> {
        let mut ids = ids;
        if let Some(ids) = &mut ids {
            ids.sort();
            ids.dedup();
        }
        let mut tx = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| storage_failure(&error))?;
        Self::lock_protection_barrier(&mut tx).await?;
        if let Some(existing) = Self::existing_protection(&mut tx, &key).await? {
            if existing.is_released()
                || ids.as_ref().is_some_and(|ids| {
                    existing
                        .records
                        .iter()
                        .map(|record| record.artifact.artifact_id())
                        .ne(ids.iter())
                })
            {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            Self::validate_protected_content(&mut tx, &existing.records).await?;
            tx.commit().await.map_err(|error| storage_failure(&error))?;
            return Ok(existing);
        }
        let records = Self::load_protected_records(&mut tx, ids.as_deref()).await?;
        let snapshot = ArtifactSnapshot::protected(key, records);
        Self::persist_protection(&mut tx, &snapshot).await?;
        tx.commit().await.map_err(|error| storage_failure(&error))?;
        Ok(snapshot)
    }

    pub(super) async fn protect_exact_records(
        &self,
        key: ArtifactIdempotencyKey,
        mut records: Vec<ArtifactRecord>,
    ) -> Result<ArtifactSnapshot, ArtifactStoreError> {
        records.sort_by(|left, right| {
            left.artifact
                .artifact_id()
                .cmp(right.artifact.artifact_id())
        });
        if records
            .windows(2)
            .any(|pair| pair[0].artifact.artifact_id() == pair[1].artifact.artifact_id())
        {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        let mut tx = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| storage_failure(&error))?;
        Self::lock_protection_barrier(&mut tx).await?;
        if let Some(existing) = Self::existing_protection(&mut tx, &key).await? {
            if existing.records != records {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            tx.commit().await.map_err(|error| storage_failure(&error))?;
            return Ok(existing);
        }
        let snapshot = ArtifactSnapshot::protected(key, records);
        Self::persist_protection(&mut tx, &snapshot).await?;
        tx.commit().await.map_err(|error| storage_failure(&error))?;
        Ok(snapshot)
    }
}
