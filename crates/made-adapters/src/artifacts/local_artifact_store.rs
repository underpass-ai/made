use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use made_core::ports::{
    ArtifactChunkPage, ArtifactPage, ArtifactPageLimit, ArtifactRecord, ArtifactStoreError,
    ArtifactStorePort, ArtifactTombstone, ArtifactUploadId, ArtifactUploadStatus,
    BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact,
    ARTIFACT_MAX_BYTES, ARTIFACT_MAX_CHUNK_BYTES,
};
use made_core::value_objects::{ArtifactId, ArtifactRef};
use uuid::Uuid;

use super::hashing::{digest_bytes, digest_reader, stable_key};
use super::local_upload_manifest::LocalUploadManifest;
use super::local_upload_state::LocalUploadState;

/// Durable single-host artifact store with atomic publication on one filesystem.
#[derive(Debug, Clone)]
pub struct LocalArtifactStore {
    root: PathBuf,
    gate: Arc<Mutex<()>>,
}

impl LocalArtifactStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ArtifactStoreError> {
        let root = root.as_ref().to_path_buf();
        for child in ["uploads", "idempotency", "artifacts", "blobs"] {
            fs::create_dir_all(root.join(child)).map_err(storage_failure)?;
        }
        Ok(Self {
            root,
            gate: Arc::new(Mutex::new(())),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, ()>, ArtifactStoreError> {
        self.gate
            .lock()
            .map_err(|_| ArtifactStoreError::StorageUnavailable)
    }

    fn upload_manifest_path(&self, id: &ArtifactUploadId) -> PathBuf {
        self.root
            .join("uploads")
            .join(format!("{}.json", stable_key(id.as_str())))
    }

    fn upload_part_path(&self, id: &ArtifactUploadId) -> PathBuf {
        self.root
            .join("uploads")
            .join(format!("{}.part", stable_key(id.as_str())))
    }

    fn idempotency_path(&self, key: &str) -> PathBuf {
        self.root
            .join("idempotency")
            .join(format!("{}.json", stable_key(key)))
    }

    fn artifact_path(&self, id: &ArtifactId) -> PathBuf {
        self.root
            .join("artifacts")
            .join(format!("{}.json", stable_key(id.as_str())))
    }

    fn blob_path(&self, artifact: &ArtifactRef) -> PathBuf {
        self.root
            .join("blobs")
            .join(artifact.digest().as_str().trim_start_matches("sha256:"))
    }

    fn load_upload(
        &self,
        id: &ArtifactUploadId,
    ) -> Result<LocalUploadManifest, ArtifactStoreError> {
        read_json(&self.upload_manifest_path(id))?.ok_or(ArtifactStoreError::NotFound)
    }

    fn upload_status(
        &self,
        manifest: &LocalUploadManifest,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        let next_offset = match &manifest.state {
            LocalUploadState::Active => fs::metadata(self.upload_part_path(&manifest.upload_id))
                .map_err(storage_failure)?
                .len(),
            LocalUploadState::Committed(artifact) => artifact.size_bytes().get(),
            LocalUploadState::Aborted => 0,
        };
        Ok(ArtifactUploadStatus {
            upload_id: manifest.upload_id.clone(),
            next_offset,
            chunk_limit_bytes: ARTIFACT_MAX_CHUNK_BYTES,
        })
    }

    fn load_record(&self, id: &ArtifactId) -> Result<ArtifactRecord, ArtifactStoreError> {
        read_json(&self.artifact_path(id))?.ok_or(ArtifactStoreError::NotFound)
    }
}

#[async_trait]
impl ArtifactStorePort for LocalArtifactStore {
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
        let _guard = self.lock()?;
        let idempotency_path = self.idempotency_path(request.idempotency_key.as_str());
        if let Some(upload_id) = read_json::<ArtifactUploadId>(&idempotency_path)? {
            let manifest = self.load_upload(&upload_id)?;
            if manifest.request != request {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            if matches!(manifest.state, LocalUploadState::Aborted) {
                return Err(ArtifactStoreError::UploadAborted);
            }
            return self.upload_status(&manifest);
        }

        let upload_id = ArtifactUploadId::new(format!("upload-{}", Uuid::new_v4()))?;
        let manifest = LocalUploadManifest {
            upload_id: upload_id.clone(),
            request,
            state: LocalUploadState::Active,
        };
        File::create(self.upload_part_path(&upload_id))
            .and_then(|file| file.sync_all())
            .map_err(storage_failure)?;
        write_json_atomic(&self.upload_manifest_path(&upload_id), &manifest)?;
        write_json_atomic(&idempotency_path, &upload_id)?;
        self.upload_status(&manifest)
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
        let _guard = self.lock()?;
        let manifest = self.load_upload(&request.upload_id)?;
        match manifest.state {
            LocalUploadState::Committed(_) => return Err(ArtifactStoreError::UploadCommitted),
            LocalUploadState::Aborted => return Err(ArtifactStoreError::UploadAborted),
            LocalUploadState::Active => {}
        }
        let part_path = self.upload_part_path(&request.upload_id);
        let stored = fs::metadata(&part_path).map_err(storage_failure)?.len();
        if request.offset < stored {
            let chunk_end = request.offset.saturating_add(request.bytes.len() as u64);
            if chunk_end > stored {
                return Err(ArtifactStoreError::UnexpectedOffset {
                    expected: stored,
                    actual: request.offset,
                });
            }
            let mut file = File::open(part_path).map_err(storage_failure)?;
            file.seek(SeekFrom::Start(request.offset))
                .map_err(storage_failure)?;
            let mut persisted = vec![0; request.bytes.len()];
            file.read_exact(&mut persisted).map_err(storage_failure)?;
            if persisted != request.bytes {
                return Err(ArtifactStoreError::UnexpectedOffset {
                    expected: stored,
                    actual: request.offset,
                });
            }
            return self.upload_status(&manifest);
        }
        if request.offset != stored {
            return Err(ArtifactStoreError::UnexpectedOffset {
                expected: stored,
                actual: request.offset,
            });
        }
        let next = stored.saturating_add(request.bytes.len() as u64);
        if next > manifest.request.size_bytes.get() {
            return Err(ArtifactStoreError::UnexpectedOffset {
                expected: manifest.request.size_bytes.get(),
                actual: next,
            });
        }
        let mut file = OpenOptions::new()
            .append(true)
            .open(part_path)
            .map_err(storage_failure)?;
        file.write_all(&request.bytes)
            .and_then(|()| file.sync_all())
            .map_err(storage_failure)?;
        self.upload_status(&manifest)
    }

    async fn commit_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        let _guard = self.lock()?;
        let mut manifest = self.load_upload(upload_id)?;
        match &manifest.state {
            LocalUploadState::Committed(artifact) => return Ok(artifact.clone()),
            LocalUploadState::Aborted => return Err(ArtifactStoreError::UploadAborted),
            LocalUploadState::Active => {}
        }
        let part_path = self.upload_part_path(upload_id);
        let (digest, size) = digest_reader(File::open(&part_path).map_err(storage_failure)?)
            .map_err(storage_failure)?;
        if size != manifest.request.size_bytes.get() {
            return Err(ArtifactStoreError::Incomplete {
                expected: manifest.request.size_bytes.get(),
                actual: size,
            });
        }
        if digest != manifest.request.expected_digest {
            return Err(ArtifactStoreError::FinalDigestMismatch);
        }
        let artifact_id = match &manifest.request.requested_artifact_id {
            Some(requested) => requested.clone(),
            None => ArtifactId::new(format!(
                "artifact-{}",
                upload_id.as_str().trim_start_matches("upload-")
            ))?,
        };
        let artifact = ArtifactRef::new(
            artifact_id.clone(),
            digest,
            manifest.request.size_bytes,
            manifest.request.media_type.clone(),
            manifest.request.provenance.clone(),
        );
        let blob_path = self.blob_path(&artifact);
        if blob_path.exists() {
            let (existing_digest, existing_size) =
                digest_reader(File::open(&blob_path).map_err(storage_failure)?)
                    .map_err(storage_failure)?;
            if existing_digest != *artifact.digest() || existing_size != artifact.size_bytes().get()
            {
                return Err(ArtifactStoreError::FinalDigestMismatch);
            }
            fs::remove_file(&part_path).map_err(storage_failure)?;
        } else {
            fs::rename(&part_path, &blob_path).map_err(storage_failure)?;
            sync_directory(blob_path.parent().expect("blob directory"))?;
        }
        if let Ok(existing) = self.load_record(&artifact_id) {
            if existing.artifact != artifact {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
        }
        let record = ArtifactRecord {
            artifact: artifact.clone(),
            tombstone: None,
        };
        write_json_atomic(&self.artifact_path(&artifact_id), &record)?;
        manifest.state = LocalUploadState::Committed(artifact.clone());
        write_json_atomic(&self.upload_manifest_path(upload_id), &manifest)?;
        Ok(artifact)
    }

    async fn abort_upload(&self, upload_id: &ArtifactUploadId) -> Result<(), ArtifactStoreError> {
        let _guard = self.lock()?;
        let mut manifest = self.load_upload(upload_id)?;
        match manifest.state {
            LocalUploadState::Committed(_) => return Err(ArtifactStoreError::UploadCommitted),
            LocalUploadState::Aborted => return Ok(()),
            LocalUploadState::Active => {}
        }
        let part = self.upload_part_path(upload_id);
        if part.exists() {
            fs::remove_file(part).map_err(storage_failure)?;
        }
        manifest.state = LocalUploadState::Aborted;
        write_json_atomic(&self.upload_manifest_path(upload_id), &manifest)
    }

    async fn get(&self, artifact_id: &ArtifactId) -> Result<ArtifactRecord, ArtifactStoreError> {
        let _guard = self.lock()?;
        self.load_record(artifact_id)
    }

    async fn list(
        &self,
        after: Option<&ArtifactId>,
        limit: ArtifactPageLimit,
    ) -> Result<ArtifactPage, ArtifactStoreError> {
        let _guard = self.lock()?;
        let mut records = Vec::new();
        for entry in fs::read_dir(self.root.join("artifacts")).map_err(storage_failure)? {
            let path = entry.map_err(storage_failure)?.path();
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                if let Some(record) = read_json::<ArtifactRecord>(&path)? {
                    records.push(record);
                }
            }
        }
        records.sort_by(|left, right| {
            left.artifact
                .artifact_id()
                .cmp(right.artifact.artifact_id())
        });
        if let Some(after) = after {
            records.retain(|record| record.artifact.artifact_id() > after);
        }
        let maximum = usize::from(limit.get());
        let has_more = records.len() > maximum;
        records.truncate(maximum);
        let next_after = has_more.then(|| {
            records
                .last()
                .expect("a nonzero page has a last item")
                .artifact
                .artifact_id()
                .clone()
        });
        Ok(ArtifactPage {
            items: records,
            next_after,
        })
    }

    async fn read_chunk(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        self.read_chunk_inner(&request, false)
    }

    async fn read_chunk_for_backup(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        self.read_chunk_inner(&request, true)
    }

    async fn tombstone(
        &self,
        command: TombstoneArtifact,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        let _guard = self.lock()?;
        let mut record = self.load_record(&command.artifact_id)?;
        let tombstone = ArtifactTombstone {
            actor: command.actor,
            policy: command.policy,
            retired_at: command.retired_at,
            digest: record.artifact.digest().clone(),
        };
        if let Some(existing) = &record.tombstone {
            return if existing == &tombstone {
                Ok(existing.clone())
            } else {
                Err(ArtifactStoreError::IdempotencyConflict)
            };
        }
        record.tombstone = Some(tombstone.clone());
        write_json_atomic(&self.artifact_path(&command.artifact_id), &record)?;
        Ok(tombstone)
    }
}

impl LocalArtifactStore {
    fn read_chunk_inner(
        &self,
        request: &ReadArtifactChunk,
        include_tombstoned: bool,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        if request.max_bytes == 0 || request.max_bytes > ARTIFACT_MAX_CHUNK_BYTES {
            return Err(ArtifactStoreError::ChunkTooLarge {
                actual: request.max_bytes as usize,
                max: ARTIFACT_MAX_CHUNK_BYTES,
            });
        }
        let _guard = self.lock()?;
        let record = self.load_record(&request.artifact_id)?;
        if !include_tombstoned && record.tombstone.is_some() {
            return Err(ArtifactStoreError::Tombstoned);
        }
        let size = record.artifact.size_bytes().get();
        if request.offset > size {
            return Err(ArtifactStoreError::UnexpectedOffset {
                expected: size,
                actual: request.offset,
            });
        }
        let mut file = File::open(self.blob_path(&record.artifact)).map_err(storage_failure)?;
        file.seek(SeekFrom::Start(request.offset))
            .map_err(storage_failure)?;
        let remaining = size - request.offset;
        let amount = remaining.min(u64::from(request.max_bytes)) as usize;
        let mut bytes = vec![0; amount];
        file.read_exact(&mut bytes).map_err(storage_failure)?;
        let next_offset = request.offset + amount as u64;
        Ok(ArtifactChunkPage {
            chunk_digest: digest_bytes(&bytes),
            bytes,
            next_offset,
            eof: next_offset == size,
        })
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, ArtifactStoreError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| ArtifactStoreError::StorageUnavailable),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(storage_failure(error)),
    }
}

fn write_json_atomic(path: &Path, value: &impl serde::Serialize) -> Result<(), ArtifactStoreError> {
    let bytes = serde_json::to_vec(value).map_err(|_| ArtifactStoreError::StorageUnavailable)?;
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let mut file = File::create(&temporary).map_err(storage_failure)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(storage_failure)?;
    fs::rename(&temporary, path).map_err(storage_failure)?;
    sync_directory(path.parent().expect("artifact state has a parent"))
}

fn sync_directory(path: &Path) -> Result<(), ArtifactStoreError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(storage_failure)
}

fn storage_failure(error: std::io::Error) -> ArtifactStoreError {
    tracing::error!(%error, "local artifact store operation failed");
    drop(error);
    ArtifactStoreError::StorageUnavailable
}
