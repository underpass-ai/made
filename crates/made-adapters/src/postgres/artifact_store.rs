use async_trait::async_trait;
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactChunkPage, ArtifactIdempotencyKey,
    ArtifactPage, ArtifactPageLimit, ArtifactRecord, ArtifactSnapshot, ArtifactStoreError,
    ArtifactStorePort, ArtifactTombstone, ArtifactUploadId, ArtifactUploadStatus,
    BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact,
    ARTIFACT_MAX_BYTES, ARTIFACT_MAX_CHUNK_BYTES,
};
use made_core::value_objects::{ArtifactId, ArtifactRef, AuthorizationEvidence};
use serde_json::Value as JsonValue;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::artifacts::hashing::digest_bytes;

use super::artifact_blob_queries::{hash_blob_chunks, hash_upload_chunks, persist_canonical_blob};
use super::PostgresPool;

/// Replica-safe artifact store backed by transactional Postgres chunks.
#[derive(Debug, Clone)]
pub struct PostgresArtifactStore {
    pub(super) pool: PostgresPool,
}

impl PostgresArtifactStore {
    #[must_use]
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    async fn locked_upload(
        tx: &mut Transaction<'_, Postgres>,
        upload_id: &ArtifactUploadId,
    ) -> Result<(BeginArtifactUpload, String, Option<ArtifactRef>), ArtifactStoreError> {
        let row = sqlx::query(
            "SELECT request, state, artifact FROM artifact_uploads WHERE upload_id = $1 FOR UPDATE",
        )
        .bind(upload_id.as_str())
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_failure)?
        .ok_or(ArtifactStoreError::NotFound)?;
        let request = serde_json::from_value(
            row.try_get::<JsonValue, _>("request")
                .map_err(storage_failure)?,
        )
        .map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        let state = row.try_get("state").map_err(storage_failure)?;
        let artifact = row
            .try_get::<Option<JsonValue>, _>("artifact")
            .map_err(storage_failure)?
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        Ok((request, state, artifact))
    }

    async fn next_offset(
        tx: &mut Transaction<'_, Postgres>,
        upload_id: &ArtifactUploadId,
    ) -> Result<u64, ArtifactStoreError> {
        let row = sqlx::query(
            "SELECT COALESCE(MAX(chunk_offset + OCTET_LENGTH(bytes)), 0) AS next_offset FROM artifact_upload_chunks WHERE upload_id = $1",
        )
        .bind(upload_id.as_str())
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_failure)?;
        to_u64(
            row.try_get::<i64, _>("next_offset")
                .map_err(storage_failure)?,
        )
    }
}

#[async_trait]
impl ArtifactStorePort for PostgresArtifactStore {
    async fn protect_snapshot(
        &self,
        key: ArtifactIdempotencyKey,
    ) -> Result<ArtifactSnapshot, ArtifactStoreError> {
        self.protect_records(key, None).await
    }

    async fn protect_references(
        &self,
        key: ArtifactIdempotencyKey,
        ids: Vec<ArtifactId>,
    ) -> Result<ArtifactSnapshot, ArtifactStoreError> {
        self.protect_records(key, Some(ids)).await
    }

    async fn protect_restore(
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
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        Self::lock_protection_barrier(&mut tx).await?;
        if let Some(existing) = Self::existing_protection(&mut tx, &key).await? {
            if existing.is_released() || existing.records != records {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            tx.commit().await.map_err(storage_failure)?;
            return Ok(existing);
        }
        let snapshot = ArtifactSnapshot::protected(key, records);
        Self::persist_protection(&mut tx, &snapshot).await?;
        tx.commit().await.map_err(storage_failure)?;
        Ok(snapshot)
    }

    async fn release_snapshot(
        &self,
        key: &ArtifactIdempotencyKey,
    ) -> Result<(), ArtifactStoreError> {
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        Self::lock_protection_barrier(&mut tx).await?;
        let mut snapshot = Self::existing_protection(&mut tx, key)
            .await?
            .ok_or(ArtifactStoreError::NotFound)?;
        if snapshot.is_released() {
            tx.commit().await.map_err(storage_failure)?;
            return Ok(());
        }
        snapshot.release();
        sqlx::query(
            "UPDATE artifact_protections SET body = $2, state = 'released', updated_at = NOW() WHERE protection_key = $1",
        )
        .bind(key.as_str())
        .bind(serde_json::to_value(snapshot).map_err(|_| ArtifactStoreError::StorageUnavailable)?)
        .execute(&mut *tx)
        .await
        .map_err(storage_failure)?;
        tx.commit().await.map_err(storage_failure)
    }

    async fn begin_upload(
        &self,
        request: BeginArtifactUpload,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        if request.size_bytes.get() > ARTIFACT_MAX_BYTES {
            return Err(ArtifactStoreError::ArtifactTooLarge {
                actual: request.size_bytes.get(),
                max: ARTIFACT_MAX_BYTES,
            });
        }
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        if let Some(row) = sqlx::query(
            "SELECT upload_id, request, state FROM artifact_uploads WHERE idempotency_key = $1 FOR UPDATE",
        )
        .bind(request.idempotency_key.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_failure)?
        {
            let existing: BeginArtifactUpload = serde_json::from_value(
                row.try_get::<JsonValue, _>("request").map_err(storage_failure)?,
            )
            .map_err(|_| ArtifactStoreError::StorageUnavailable)?;
            if existing != request {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            let state: String = row.try_get("state").map_err(storage_failure)?;
            if state == "aborted" {
                return Err(ArtifactStoreError::UploadAborted);
            }
            let upload_id = ArtifactUploadId::new(row.try_get::<String, _>("upload_id").map_err(storage_failure)?)?;
            let next_offset = Self::next_offset(&mut tx, &upload_id).await?;
            tx.commit().await.map_err(storage_failure)?;
            return Ok(status(upload_id, next_offset));
        }
        let upload_id = ArtifactUploadId::new(format!("upload-{}", Uuid::new_v4()))?;
        let request_json =
            serde_json::to_value(&request).map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        sqlx::query("INSERT INTO artifact_uploads (upload_id, idempotency_key, request, state) VALUES ($1, $2, $3, 'active')")
            .bind(upload_id.as_str())
            .bind(request.idempotency_key.as_str())
            .bind(request_json)
            .execute(&mut *tx)
            .await
            .map_err(storage_failure)?;
        tx.commit().await.map_err(storage_failure)?;
        Ok(status(upload_id, 0))
    }

    async fn put_chunk(
        &self,
        request: PutArtifactChunk,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        if request.bytes.is_empty() {
            return Err(made_core::DomainError::MustBeNonZero {
                field: "artifact_chunk_bytes",
            }
            .into());
        }
        if request.bytes.len() > ARTIFACT_MAX_CHUNK_BYTES as usize {
            return Err(ArtifactStoreError::ChunkTooLarge {
                actual: request.bytes.len(),
                max: ARTIFACT_MAX_CHUNK_BYTES,
            });
        }
        if digest_bytes(&request.bytes) != request.chunk_digest {
            return Err(ArtifactStoreError::ChunkDigestMismatch);
        }
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        let (begin, state, _) = Self::locked_upload(&mut tx, &request.upload_id).await?;
        match state.as_str() {
            "committed" => return Err(ArtifactStoreError::UploadCommitted),
            "aborted" => return Err(ArtifactStoreError::UploadAborted),
            "active" => {}
            _ => return Err(ArtifactStoreError::StorageUnavailable),
        }
        let next_offset = Self::next_offset(&mut tx, &request.upload_id).await?;
        let offset = request.offset.get();
        if offset < next_offset {
            let existing = sqlx::query("SELECT bytes, chunk_digest FROM artifact_upload_chunks WHERE upload_id = $1 AND chunk_offset = $2")
                .bind(request.upload_id.as_str())
                .bind(to_i64(offset)?)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_failure)?;
            if let Some(existing) = existing {
                let bytes: Vec<u8> = existing.try_get("bytes").map_err(storage_failure)?;
                let chunk_digest: String =
                    existing.try_get("chunk_digest").map_err(storage_failure)?;
                if bytes == request.bytes && chunk_digest == request.chunk_digest.as_str() {
                    tx.commit().await.map_err(storage_failure)?;
                    return Ok(status(request.upload_id, next_offset));
                }
            }
            return Err(ArtifactStoreError::UnexpectedOffset {
                expected: next_offset,
                actual: offset,
            });
        }
        if offset != next_offset {
            return Err(ArtifactStoreError::UnexpectedOffset {
                expected: next_offset,
                actual: offset,
            });
        }
        let new_offset = next_offset.saturating_add(request.bytes.len() as u64);
        if new_offset > begin.size_bytes.get() {
            return Err(ArtifactStoreError::UnexpectedOffset {
                expected: begin.size_bytes.get(),
                actual: new_offset,
            });
        }
        sqlx::query("INSERT INTO artifact_upload_chunks (upload_id, chunk_offset, bytes, chunk_digest) VALUES ($1, $2, $3, $4)")
            .bind(request.upload_id.as_str())
            .bind(to_i64(offset)?)
            .bind(request.bytes)
            .bind(request.chunk_digest.as_str())
            .execute(&mut *tx)
            .await
            .map_err(storage_failure)?;
        tx.commit().await.map_err(storage_failure)?;
        Ok(status(request.upload_id, new_offset))
    }

    async fn artifact_id_for_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactId, ArtifactStoreError> {
        let row = sqlx::query("SELECT request FROM artifact_uploads WHERE upload_id = $1")
            .bind(upload_id.as_str())
            .fetch_optional(self.pool.inner())
            .await
            .map_err(storage_failure)?
            .ok_or(ArtifactStoreError::NotFound)?;
        let request: BeginArtifactUpload = serde_json::from_value(
            row.try_get::<JsonValue, _>("request")
                .map_err(storage_failure)?,
        )
        .map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        request.requested_artifact_id.map_or_else(
            || {
                ArtifactId::new(format!(
                    "artifact-{}",
                    upload_id.as_str().trim_start_matches("upload-")
                ))
                .map_err(Into::into)
            },
            Ok,
        )
    }

    async fn commit_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        self.commit_upload_authorized(upload_id, None).await
    }

    async fn commit_upload_authorized(
        &self,
        upload_id: &ArtifactUploadId,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        Self::lock_protection_barrier(&mut tx).await?;
        let (request, state, artifact) = Self::locked_upload(&mut tx, upload_id).await?;
        if state == "committed" {
            return artifact.ok_or(ArtifactStoreError::StorageUnavailable);
        }
        if state == "aborted" {
            return Err(ArtifactStoreError::UploadAborted);
        }
        let (digest, size) = hash_upload_chunks(&mut tx, upload_id).await?;
        if size != request.size_bytes.get() {
            return Err(ArtifactStoreError::Incomplete {
                expected: request.size_bytes.get(),
                actual: size,
            });
        }
        if digest != request.expected_digest.as_str() {
            return Err(ArtifactStoreError::FinalDigestMismatch);
        }
        persist_canonical_blob(&mut tx, upload_id, request.expected_digest.as_str(), size).await?;
        let (stored_digest, stored_size) =
            hash_blob_chunks(&mut tx, request.expected_digest.as_str(), size).await?;
        if stored_digest != request.expected_digest.as_str() || stored_size != size {
            return Err(ArtifactStoreError::FinalDigestMismatch);
        }
        let artifact_id = request
            .requested_artifact_id
            .clone()
            .unwrap_or(ArtifactId::new(format!(
                "artifact-{}",
                upload_id.as_str().trim_start_matches("upload-")
            ))?);
        let artifact = ArtifactRef::new(
            artifact_id.clone(),
            request.expected_digest,
            request.size_bytes,
            request.media_type,
            request.provenance,
        );
        let record = if let Some(row) =
            sqlx::query("SELECT body FROM artifact_records WHERE artifact_id = $1 FOR UPDATE")
                .bind(artifact_id.as_str())
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_failure)?
        {
            let existing: ArtifactRecord =
                serde_json::from_value(row.try_get("body").map_err(storage_failure)?)
                    .map_err(|_| ArtifactStoreError::StorageUnavailable)?;
            if existing.artifact != artifact {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            existing
        } else {
            ArtifactRecord {
                artifact: artifact.clone(),
                tombstone: None,
                authorization,
            }
        };
        let record_json =
            serde_json::to_value(record).map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        let artifact_json =
            serde_json::to_value(&artifact).map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        sqlx::query("INSERT INTO artifact_records (artifact_id, body) VALUES ($1, $2) ON CONFLICT (artifact_id) DO UPDATE SET body = EXCLUDED.body, updated_at = NOW()")
            .bind(artifact_id.as_str())
            .bind(record_json)
            .execute(&mut *tx)
            .await
            .map_err(storage_failure)?;
        sqlx::query("UPDATE artifact_uploads SET state = 'committed', artifact = $2, updated_at = NOW() WHERE upload_id = $1")
            .bind(upload_id.as_str())
            .bind(artifact_json)
            .execute(&mut *tx)
            .await
            .map_err(storage_failure)?;
        tx.commit().await.map_err(storage_failure)?;
        Ok(artifact)
    }

    async fn abort_upload(&self, upload_id: &ArtifactUploadId) -> Result<(), ArtifactStoreError> {
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        let (_, state, _) = Self::locked_upload(&mut tx, upload_id).await?;
        if state == "committed" {
            return Err(ArtifactStoreError::UploadCommitted);
        }
        if state == "aborted" {
            return Ok(());
        }
        sqlx::query("DELETE FROM artifact_upload_chunks WHERE upload_id = $1")
            .bind(upload_id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(storage_failure)?;
        sqlx::query("UPDATE artifact_uploads SET state = 'aborted', updated_at = NOW() WHERE upload_id = $1").bind(upload_id.as_str()).execute(&mut *tx).await.map_err(storage_failure)?;
        tx.commit().await.map_err(storage_failure)
    }

    async fn get(&self, artifact_id: &ArtifactId) -> Result<ArtifactRecord, ArtifactStoreError> {
        let row = sqlx::query("SELECT body FROM artifact_records WHERE artifact_id = $1")
            .bind(artifact_id.as_str())
            .fetch_optional(self.pool.inner())
            .await
            .map_err(storage_failure)?
            .ok_or(ArtifactStoreError::NotFound)?;
        serde_json::from_value(row.try_get("body").map_err(storage_failure)?)
            .map_err(|_| ArtifactStoreError::StorageUnavailable)
    }

    async fn list(
        &self,
        after: Option<&ArtifactId>,
        limit: ArtifactPageLimit,
    ) -> Result<ArtifactPage, ArtifactStoreError> {
        let after = after.map_or("", ArtifactId::as_str);
        let rows = sqlx::query("SELECT body FROM artifact_records WHERE artifact_id > $1 ORDER BY artifact_id LIMIT $2")
            .bind(after).bind(i64::from(limit.get()) + 1).fetch_all(self.pool.inner()).await.map_err(storage_failure)?;
        let mut items: Vec<ArtifactRecord> = rows
            .into_iter()
            .map(|row| {
                serde_json::from_value(row.try_get("body").map_err(storage_failure)?)
                    .map_err(|_| ArtifactStoreError::StorageUnavailable)
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

    async fn read_chunk(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        self.read_inner(request, false).await
    }

    async fn read_chunk_for_backup(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        self.read_inner(request, true).await
    }

    async fn backup_content_available(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<bool, ArtifactStoreError> {
        let record = self.get(artifact_id).await?;
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        let (digest, size) = hash_blob_chunks(
            &mut tx,
            record.artifact.digest().as_str(),
            record.artifact.size_bytes().get(),
        )
        .await?;
        tx.commit().await.map_err(storage_failure)?;
        if digest == record.artifact.digest().as_str() && size == record.artifact.size_bytes().get()
        {
            Ok(true)
        } else if record.tombstone.is_some() && size == 0 {
            Ok(false)
        } else {
            Err(ArtifactStoreError::FinalDigestMismatch)
        }
    }

    async fn restore_retired_metadata(
        &self,
        record: ArtifactRecord,
    ) -> Result<(), ArtifactStoreError> {
        if record.tombstone.is_none() {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        let body =
            serde_json::to_value(&record).map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        let result = sqlx::query(
            "INSERT INTO artifact_records (artifact_id, body) VALUES ($1, $2) ON CONFLICT (artifact_id) DO NOTHING",
        )
        .bind(record.artifact.artifact_id().as_str())
        .bind(body)
        .execute(self.pool.inner())
        .await
        .map_err(storage_failure)?;
        if result.rows_affected() == 0 && self.get(record.artifact.artifact_id()).await? != record {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        Ok(())
    }

    async fn active_protections(&self) -> Result<Vec<ArtifactSnapshot>, ArtifactStoreError> {
        sqlx::query("SELECT body FROM artifact_protections WHERE state = 'protected' ORDER BY protection_key")
            .fetch_all(self.pool.inner())
            .await
            .map_err(storage_failure)?
            .into_iter()
            .map(|row| {
                serde_json::from_value(row.try_get("body").map_err(storage_failure)?)
                    .map_err(|_| ArtifactStoreError::StorageUnavailable)
            })
            .collect()
    }

    async fn tombstone(
        &self,
        command: TombstoneArtifact,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        self.tombstone_authorized(command, None).await
    }

    async fn tombstone_authorized(
        &self,
        command: TombstoneArtifact,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        let row =
            sqlx::query("SELECT body FROM artifact_records WHERE artifact_id = $1 FOR UPDATE")
                .bind(command.artifact_id.as_str())
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_failure)?
                .ok_or(ArtifactStoreError::NotFound)?;
        let mut record: ArtifactRecord =
            serde_json::from_value(row.try_get("body").map_err(storage_failure)?)
                .map_err(|_| ArtifactStoreError::StorageUnavailable)?;
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
        .bind(serde_json::to_value(record).map_err(|_| ArtifactStoreError::StorageUnavailable)?)
        .execute(&mut *tx)
        .await
        .map_err(storage_failure)?;
        tx.commit().await.map_err(storage_failure)?;
        Ok(tombstone)
    }
}

fn status(upload_id: ArtifactUploadId, next_offset: u64) -> ArtifactUploadStatus {
    ArtifactUploadStatus {
        upload_id,
        next_offset: ArtifactByteOffset::new(next_offset),
        chunk_limit: ArtifactChunkLimit::MAX,
    }
}

pub(super) fn to_i64(value: u64) -> Result<i64, ArtifactStoreError> {
    i64::try_from(value).map_err(|_| ArtifactStoreError::StorageUnavailable)
}

pub(super) fn to_u64(value: i64) -> Result<u64, ArtifactStoreError> {
    u64::try_from(value).map_err(|_| ArtifactStoreError::StorageUnavailable)
}

pub(super) fn storage_failure(error: sqlx::Error) -> ArtifactStoreError {
    tracing::error!(%error, "postgres artifact store operation failed");
    drop(error);
    ArtifactStoreError::StorageUnavailable
}
