use made_core::ports::{
    ArtifactPage, ArtifactPageLimit, ArtifactRecord, ArtifactSnapshot, ArtifactStoreError,
    ArtifactTombstone, TombstoneArtifact,
};
use made_core::value_objects::{ArtifactId, AuthorizationEvidence};
use sqlx::Row;

use super::artifact_blob_queries::hash_blob_chunks;
use super::artifact_store::{storage_failure, PostgresArtifactStore};

impl PostgresArtifactStore {
    pub(super) async fn record_by_id(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<ArtifactRecord, ArtifactStoreError> {
        let row = sqlx::query("SELECT body FROM artifact_records WHERE artifact_id = $1")
            .bind(artifact_id.as_str())
            .fetch_optional(self.pool.inner())
            .await
            .map_err(|error| storage_failure(&error))?
            .ok_or(ArtifactStoreError::NotFound)?;
        serde_json::from_value(
            row.try_get("body")
                .map_err(|error| storage_failure(&error))?,
        )
        .map_err(|error| {
            ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
        })
    }

    pub(super) async fn page_records(
        &self,
        after: Option<&ArtifactId>,
        limit: ArtifactPageLimit,
    ) -> Result<ArtifactPage, ArtifactStoreError> {
        let after = after.map_or("", ArtifactId::as_str);
        let rows = sqlx::query("SELECT body FROM artifact_records WHERE artifact_id > $1 ORDER BY artifact_id LIMIT $2")
            .bind(after).bind(i64::from(limit.get()) + 1).fetch_all(self.pool.inner()).await.map_err(|error| storage_failure(&error))?;
        let mut items: Vec<ArtifactRecord> = rows
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
        let has_more = items.len() > usize::from(limit.get());
        items.truncate(usize::from(limit.get()));
        let next_after = has_more.then(|| {
            items
                .last()
                .expect("nonzero page limit")
                .artifact
                .artifact_id()
                .clone()
        });
        Ok(ArtifactPage { items, next_after })
    }

    pub(super) async fn backup_content_available_inner(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<bool, ArtifactStoreError> {
        let record = self.record_by_id(artifact_id).await?;
        let mut tx = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| storage_failure(&error))?;
        let (digest, size) = hash_blob_chunks(
            &mut tx,
            record.artifact.digest().as_str(),
            record.artifact.size_bytes().get(),
        )
        .await?;
        tx.commit().await.map_err(|error| storage_failure(&error))?;
        if digest == record.artifact.digest().as_str() && size == record.artifact.size_bytes().get()
        {
            Ok(true)
        } else if record.tombstone.is_some() && size == 0 {
            Ok(false)
        } else {
            Err(ArtifactStoreError::FinalDigestMismatch)
        }
    }

    pub(super) async fn restore_retired_metadata_inner(
        &self,
        record: ArtifactRecord,
    ) -> Result<(), ArtifactStoreError> {
        if record.tombstone.is_none() {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        let body = serde_json::to_value(&record).map_err(|error| {
            ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
        })?;
        let result = sqlx::query(
            "INSERT INTO artifact_records (artifact_id, body) VALUES ($1, $2) ON CONFLICT (artifact_id) DO NOTHING",
        )
        .bind(record.artifact.artifact_id().as_str())
        .bind(body)
        .execute(self.pool.inner())
        .await
        .map_err(|error| storage_failure(&error))?;
        if result.rows_affected() == 0
            && self.record_by_id(record.artifact.artifact_id()).await? != record
        {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        Ok(())
    }

    pub(super) async fn active_protections_inner(
        &self,
    ) -> Result<Vec<ArtifactSnapshot>, ArtifactStoreError> {
        sqlx::query("SELECT body FROM artifact_protections WHERE state = 'protected' ORDER BY protection_key")
            .fetch_all(self.pool.inner())
            .await
            .map_err(|error| storage_failure(&error))?
            .into_iter()
            .map(|row| {
                serde_json::from_value(row.try_get("body").map_err(|error| storage_failure(&error))?)
                    .map_err(|error| ArtifactStoreError::unavailable("serialize or decode artifact metadata", error))
            })
            .collect()
    }

    pub(super) async fn tombstone_record(
        &self,
        command: TombstoneArtifact,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        let mut tx = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| storage_failure(&error))?;
        let row =
            sqlx::query("SELECT body FROM artifact_records WHERE artifact_id = $1 FOR UPDATE")
                .bind(command.artifact_id.as_str())
                .fetch_optional(&mut *tx)
                .await
                .map_err(|error| storage_failure(&error))?
                .ok_or(ArtifactStoreError::NotFound)?;
        let mut record: ArtifactRecord = serde_json::from_value(
            row.try_get("body")
                .map_err(|error| storage_failure(&error))?,
        )
        .map_err(|error| {
            ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
        })?;
        let tombstone = ArtifactTombstone {
            actor: command.actor,
            policy: command.policy,
            retired_at: command.retired_at,
            digest: record.artifact.digest().clone(),
            authorization,
        };
        if let Some(existing) = &record.tombstone {
            return if existing.same_retirement_as(&tombstone) {
                Ok(existing.clone())
            } else {
                Err(ArtifactStoreError::IdempotencyConflict)
            };
        }
        record.tombstone = Some(tombstone.clone());
        sqlx::query(
            "UPDATE artifact_records SET body = $2, updated_at = NOW() WHERE artifact_id = $1",
        )
        .bind(command.artifact_id.as_str())
        .bind(serde_json::to_value(record).map_err(|error| {
            ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
        })?)
        .execute(&mut *tx)
        .await
        .map_err(|error| storage_failure(&error))?;
        tx.commit().await.map_err(|error| storage_failure(&error))?;
        Ok(tombstone)
    }
}
