use made_core::ports::{
    ArtifactStoreError, ArtifactUploadId, ARTIFACT_DEFAULT_CHUNK_BYTES, ARTIFACT_MAX_CHUNK_BYTES,
};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};

pub(super) async fn hash_upload_chunks(
    tx: &mut Transaction<'_, Postgres>,
    upload_id: &ArtifactUploadId,
) -> Result<(String, u64), ArtifactStoreError> {
    let mut hasher = Sha256::new();
    let mut next = 0_u64;
    loop {
        let row = sqlx::query("SELECT chunk_offset, bytes FROM artifact_upload_chunks WHERE upload_id = $1 AND chunk_offset >= $2 ORDER BY chunk_offset LIMIT 1")
            .bind(upload_id.as_str()).bind(to_i64(next)?).fetch_optional(&mut **tx).await.map_err(|error| storage_failure(&error))?;
        let Some(row) = row else {
            break;
        };
        let offset = to_u64(
            row.try_get("chunk_offset")
                .map_err(|error| storage_failure(&error))?,
        )?;
        if offset != next {
            return Err(ArtifactStoreError::unavailable_static(
                "artifact storage rejected the operation",
            ));
        }
        let bytes: Vec<u8> = row
            .try_get("bytes")
            .map_err(|error| storage_failure(&error))?;
        hasher.update(&bytes);
        next += bytes.len() as u64;
    }
    Ok((format!("sha256:{:x}", hasher.finalize()), next))
}

pub(super) async fn hash_blob_chunks(
    tx: &mut Transaction<'_, Postgres>,
    digest: &str,
    size: u64,
) -> Result<(String, u64), ArtifactStoreError> {
    let mut hasher = Sha256::new();
    let mut next = 0_u64;
    loop {
        let row = sqlx::query("SELECT chunk_offset, bytes FROM artifact_blobs WHERE digest = $1 AND size_bytes = $2 AND chunk_offset >= $3 ORDER BY chunk_offset LIMIT 1")
            .bind(digest).bind(to_i64(size)?).bind(to_i64(next)?).fetch_optional(&mut **tx).await.map_err(|error| storage_failure(&error))?;
        let Some(row) = row else {
            break;
        };
        let offset = to_u64(
            row.try_get("chunk_offset")
                .map_err(|error| storage_failure(&error))?,
        )?;
        if offset != next {
            return Err(ArtifactStoreError::unavailable_static(
                "artifact storage rejected the operation",
            ));
        }
        let bytes: Vec<u8> = row
            .try_get("bytes")
            .map_err(|error| storage_failure(&error))?;
        hasher.update(&bytes);
        next += bytes.len() as u64;
    }
    Ok((format!("sha256:{:x}", hasher.finalize()), next))
}

pub(super) async fn persist_canonical_blob(
    tx: &mut Transaction<'_, Postgres>,
    upload_id: &ArtifactUploadId,
    digest: &str,
    size: u64,
) -> Result<(), ArtifactStoreError> {
    let canonical = ARTIFACT_DEFAULT_CHUNK_BYTES as usize;
    let mut upload_offset = 0_u64;
    let mut blob_offset = 0_u64;
    let mut pending = Vec::with_capacity(canonical + ARTIFACT_MAX_CHUNK_BYTES as usize);
    loop {
        let row = sqlx::query("SELECT chunk_offset, bytes FROM artifact_upload_chunks WHERE upload_id = $1 AND chunk_offset >= $2 ORDER BY chunk_offset LIMIT 1")
            .bind(upload_id.as_str())
            .bind(to_i64(upload_offset)?)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|error| storage_failure(&error))?;
        let Some(row) = row else {
            break;
        };
        let offset = to_u64(
            row.try_get("chunk_offset")
                .map_err(|error| storage_failure(&error))?,
        )?;
        if offset != upload_offset {
            return Err(ArtifactStoreError::unavailable_static(
                "artifact storage rejected the operation",
            ));
        }
        let bytes: Vec<u8> = row
            .try_get("bytes")
            .map_err(|error| storage_failure(&error))?;
        upload_offset += bytes.len() as u64;
        pending.extend_from_slice(&bytes);
        while pending.len() >= canonical {
            let remainder = pending.split_off(canonical);
            insert_blob_chunk(tx, digest, size, blob_offset, &pending).await?;
            blob_offset += pending.len() as u64;
            pending = remainder;
        }
    }
    if !pending.is_empty() {
        insert_blob_chunk(tx, digest, size, blob_offset, &pending).await?;
    }
    Ok(())
}

async fn insert_blob_chunk(
    tx: &mut Transaction<'_, Postgres>,
    digest: &str,
    size: u64,
    offset: u64,
    bytes: &[u8],
) -> Result<(), ArtifactStoreError> {
    sqlx::query("INSERT INTO artifact_blobs (digest, size_bytes, chunk_offset, bytes) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING")
        .bind(digest)
        .bind(to_i64(size)?)
        .bind(to_i64(offset)?)
        .bind(bytes)
        .execute(&mut **tx)
        .await
        .map_err(|error| storage_failure(&error))?;
    let existing: Vec<u8> = sqlx::query_scalar("SELECT bytes FROM artifact_blobs WHERE digest = $1 AND size_bytes = $2 AND chunk_offset = $3")
        .bind(digest)
        .bind(to_i64(size)?)
        .bind(to_i64(offset)?)
        .fetch_one(&mut **tx)
        .await
        .map_err(|error| storage_failure(&error))?;
    if existing != bytes {
        return Err(ArtifactStoreError::FinalDigestMismatch);
    }
    Ok(())
}

fn to_i64(value: u64) -> Result<i64, ArtifactStoreError> {
    i64::try_from(value).map_err(|error| {
        ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
    })
}

fn to_u64(value: i64) -> Result<u64, ArtifactStoreError> {
    u64::try_from(value).map_err(|error| {
        ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
    })
}

fn storage_failure(error: &sqlx::Error) -> ArtifactStoreError {
    tracing::error!(%error, "postgres artifact blob operation failed");
    ArtifactStoreError::unavailable("postgres artifact blob query", error)
}
