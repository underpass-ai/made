use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkPage, ArtifactReadCompletion, ArtifactStoreError,
    ArtifactStorePort, ReadArtifactChunk,
};
use sqlx::Row;

use crate::artifacts::hashing::digest_bytes;

use super::artifact_store::{storage_failure, to_i64, to_u64, PostgresArtifactStore};

impl PostgresArtifactStore {
    pub(super) async fn read_inner(
        &self,
        request: ReadArtifactChunk,
        include_tombstoned: bool,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        let record = self.get(&request.artifact_id).await?;
        if !include_tombstoned && record.tombstone.is_some() {
            return Err(ArtifactStoreError::Tombstoned);
        }
        let size = record.artifact.size_bytes().get();
        let offset = request.offset.get();
        if offset > size {
            return Err(ArtifactStoreError::UnexpectedOffset {
                expected: size,
                actual: offset,
            });
        }
        let end = offset
            .saturating_add(u64::from(request.max_bytes.get()))
            .min(size);
        let rows = sqlx::query(
            "SELECT chunk_offset, bytes FROM artifact_blobs WHERE digest = $1 AND size_bytes = $2 AND chunk_offset < $3 AND chunk_offset + OCTET_LENGTH(bytes) > $4 ORDER BY chunk_offset",
        )
        .bind(record.artifact.digest().as_str())
        .bind(to_i64(size)?)
        .bind(to_i64(end)?)
        .bind(to_i64(offset)?)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| storage_failure(&error))?;
        let mut bytes = Vec::with_capacity((end - offset) as usize);
        for row in rows {
            let chunk_offset = to_u64(
                row.try_get::<i64, _>("chunk_offset")
                    .map_err(|error| storage_failure(&error))?,
            )?;
            let chunk: Vec<u8> = row.try_get("bytes").map_err(|error| storage_failure(&error))?;
            let start_in_chunk = offset.saturating_sub(chunk_offset) as usize;
            let end_in_chunk = ((end - chunk_offset) as usize).min(chunk.len());
            if start_in_chunk < end_in_chunk {
                bytes.extend_from_slice(&chunk[start_in_chunk..end_in_chunk]);
            }
        }
        if bytes.len() != (end - offset) as usize {
            return Err(ArtifactStoreError::unavailable_static("artifact storage rejected the operation"));
        }
        Ok(ArtifactChunkPage {
            chunk_digest: digest_bytes(&bytes),
            bytes,
            next_offset: ArtifactByteOffset::new(end),
            completion: if end == size {
                ArtifactReadCompletion::Complete
            } else {
                ArtifactReadCompletion::More
            },
        })
    }
}
