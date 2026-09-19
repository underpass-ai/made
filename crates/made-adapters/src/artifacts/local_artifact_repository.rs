use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

use fs2::FileExt;
use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactChunkPage, ArtifactPage, ArtifactPageLimit,
    ArtifactReadCompletion, ArtifactRecord, ArtifactStoreError, ArtifactTombstone,
    ArtifactUploadId, ArtifactUploadStatus, BeginArtifactUpload, PutArtifactChunk,
    ReadArtifactChunk, TombstoneArtifact, ARTIFACT_MAX_BYTES, ARTIFACT_MAX_CHUNK_BYTES,
};
use made_core::value_objects::{ArtifactId, ArtifactRef, AuthorizationEvidence};
use uuid::Uuid;

use super::hashing::{digest_bytes, digest_reader};
use super::local_artifact_io::{read_json, storage_failure, sync_directory, write_json_atomic};
use super::local_artifact_layout::LocalArtifactLayout;
use super::local_upload_manifest::LocalUploadManifest;
use super::local_upload_state::LocalUploadState;

/// Blocking transactional repository behind the async local adapter.
#[derive(Debug)]
pub(super) struct LocalArtifactRepository {
    pub(super) layout: LocalArtifactLayout,
}

impl LocalArtifactRepository {
    pub(super) fn store_identity(
        &self,
    ) -> Result<made_core::value_objects::ArtifactDigest, ArtifactStoreError> {
        let root = self
            .layout
            .artifacts_dir()
            .parent()
            .ok_or(ArtifactStoreError::StorageUnavailable)?
            .to_path_buf();
        let canonical = std::fs::canonicalize(root).map_err(storage_failure)?;
        Ok(super::hashing::digest_bytes(
            canonical.to_string_lossy().as_bytes(),
        ))
    }
    pub(super) fn restore_retired_metadata(
        &self,
        record: &ArtifactRecord,
    ) -> Result<(), ArtifactStoreError> {
        if record.tombstone.is_none() {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        self.locked(|| match self.load_record(record.artifact.artifact_id()) {
            Ok(existing) if &existing == record => Ok(()),
            Ok(_) => Err(ArtifactStoreError::IdempotencyConflict),
            Err(ArtifactStoreError::NotFound) => {
                write_json_atomic(&self.layout.artifact(record.artifact.artifact_id()), record)
            }
            Err(error) => Err(error),
        })
    }
    pub(super) fn open(root: impl AsRef<std::path::Path>) -> Result<Self, ArtifactStoreError> {
        Ok(Self {
            layout: LocalArtifactLayout::open(root)?,
        })
    }

    pub(super) fn locked<T>(
        &self,
        operation: impl FnOnce() -> Result<T, ArtifactStoreError>,
    ) -> Result<T, ArtifactStoreError> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.layout.lock_path())
            .map_err(storage_failure)?;
        FileExt::lock_exclusive(&lock).map_err(storage_failure)?;
        operation()
    }

    pub(super) fn begin(
        &self,
        request: BeginArtifactUpload,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        self.locked(|| self.begin_locked(request))
    }

    fn begin_locked(
        &self,
        request: BeginArtifactUpload,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        if request.size_bytes.get() > ARTIFACT_MAX_BYTES {
            return Err(ArtifactStoreError::ArtifactTooLarge {
                actual: request.size_bytes.get(),
                max: ARTIFACT_MAX_BYTES,
            });
        }
        if let Some(upload_id) = read_json::<ArtifactUploadId>(
            &self.layout.idempotency(request.idempotency_key.as_str()),
        )? {
            return self.resume_manifest(&request, &self.load_upload(&upload_id)?);
        }
        if let Some(manifest) = self.find_by_idempotency(request.idempotency_key.as_str())? {
            write_json_atomic(
                &self.layout.idempotency(request.idempotency_key.as_str()),
                &manifest.upload_id,
            )?;
            return self.resume_manifest(&request, &manifest);
        }

        let upload_id = ArtifactUploadId::new(format!("upload-{}", Uuid::new_v4()))?;
        let manifest = LocalUploadManifest {
            upload_id: upload_id.clone(),
            request,
            state: LocalUploadState::Active,
        };
        File::create(self.layout.upload_part(&upload_id))
            .and_then(|file| file.sync_all())
            .map_err(storage_failure)?;
        sync_directory(&self.layout.uploads_dir())?;
        write_json_atomic(&self.layout.upload_manifest(&upload_id), &manifest)?;
        write_json_atomic(
            &self
                .layout
                .idempotency(manifest.request.idempotency_key.as_str()),
            &upload_id,
        )?;
        self.status(&manifest)
    }

    fn resume_manifest(
        &self,
        request: &BeginArtifactUpload,
        manifest: &LocalUploadManifest,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        if &manifest.request != request {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        if matches!(manifest.state, LocalUploadState::Aborted) {
            return Err(ArtifactStoreError::UploadAborted);
        }
        self.status(manifest)
    }

    fn find_by_idempotency(
        &self,
        key: &str,
    ) -> Result<Option<LocalUploadManifest>, ArtifactStoreError> {
        for entry in fs::read_dir(self.layout.uploads_dir()).map_err(storage_failure)? {
            let path = entry.map_err(storage_failure)?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let manifest = read_json::<LocalUploadManifest>(&path)?
                .ok_or(ArtifactStoreError::StorageUnavailable)?;
            if manifest.request.idempotency_key.as_str() == key {
                return Ok(Some(manifest));
            }
        }
        Ok(None)
    }

    pub(super) fn put(
        &self,
        request: &PutArtifactChunk,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        self.locked(|| self.put_locked(request))
    }

    fn put_locked(
        &self,
        request: &PutArtifactChunk,
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
        let manifest = self.load_upload(&request.upload_id)?;
        match manifest.state {
            LocalUploadState::Committed(_) => return Err(ArtifactStoreError::UploadCommitted),
            LocalUploadState::Aborted => return Err(ArtifactStoreError::UploadAborted),
            LocalUploadState::Active => {}
        }
        let part_path = self.layout.upload_part(&request.upload_id);
        let stored = fs::metadata(&part_path).map_err(storage_failure)?.len();
        let offset = request.offset.get();
        if offset < stored {
            let chunk_end = offset.saturating_add(request.bytes.len() as u64);
            if chunk_end > stored {
                return Err(unexpected_offset(stored, offset));
            }
            let mut file = File::open(part_path).map_err(storage_failure)?;
            file.seek(SeekFrom::Start(offset))
                .map_err(storage_failure)?;
            let mut persisted = vec![0; request.bytes.len()];
            file.read_exact(&mut persisted).map_err(storage_failure)?;
            if persisted != request.bytes {
                return Err(unexpected_offset(stored, offset));
            }
            return self.status(&manifest);
        }
        if offset != stored {
            return Err(unexpected_offset(stored, offset));
        }
        let next = stored.saturating_add(request.bytes.len() as u64);
        if next > manifest.request.size_bytes.get() {
            return Err(unexpected_offset(manifest.request.size_bytes.get(), next));
        }
        let mut file = OpenOptions::new()
            .append(true)
            .open(part_path)
            .map_err(storage_failure)?;
        file.write_all(&request.bytes)
            .and_then(|()| file.sync_all())
            .map_err(storage_failure)?;
        self.status(&manifest)
    }

    pub(super) fn commit(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        self.commit_authorized(upload_id, None)
    }

    pub(super) fn commit_authorized(
        &self,
        upload_id: &ArtifactUploadId,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        self.locked(|| self.commit_locked(upload_id, authorization, &mut |_| Ok(())))
    }

    #[cfg(test)]
    pub(super) fn commit_observing(
        &self,
        upload_id: &ArtifactUploadId,
        mut observer: impl FnMut(&'static str) -> Result<(), ArtifactStoreError>,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        self.locked(|| self.commit_locked(upload_id, None, &mut observer))
    }

    fn commit_locked(
        &self,
        upload_id: &ArtifactUploadId,
        authorization: Option<AuthorizationEvidence>,
        observer: &mut impl FnMut(&'static str) -> Result<(), ArtifactStoreError>,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        let mut manifest = self.load_upload(upload_id)?;
        match &manifest.state {
            LocalUploadState::Committed(artifact) => return Ok(artifact.clone()),
            LocalUploadState::Aborted => return Err(ArtifactStoreError::UploadAborted),
            LocalUploadState::Active => {}
        }
        let artifact = Self::artifact_for(&manifest)?;
        self.publish_blob(upload_id, &artifact)?;
        observer("blob_published")?;

        let record = match self.load_record(artifact.artifact_id()) {
            Ok(existing) if existing.artifact == artifact => existing,
            Ok(_) => return Err(ArtifactStoreError::IdempotencyConflict),
            Err(ArtifactStoreError::NotFound) => ArtifactRecord {
                artifact: artifact.clone(),
                tombstone: None,
                authorization,
            },
            Err(error) => return Err(error),
        };
        write_json_atomic(&self.layout.artifact(artifact.artifact_id()), &record)?;
        observer("record_published")?;

        manifest.state = LocalUploadState::Committed(artifact.clone());
        write_json_atomic(&self.layout.upload_manifest(upload_id), &manifest)?;
        observer("manifest_published")?;
        Ok(artifact)
    }

    fn publish_blob(
        &self,
        upload_id: &ArtifactUploadId,
        artifact: &ArtifactRef,
    ) -> Result<(), ArtifactStoreError> {
        let part_path = self.layout.upload_part(upload_id);
        let blob_path = self.layout.blob(artifact);
        match File::open(&part_path) {
            Ok(file) => {
                verify_reader(file, artifact)?;
                match File::open(&blob_path) {
                    Ok(existing) => {
                        verify_reader(existing, artifact)?;
                        fs::remove_file(&part_path).map_err(storage_failure)?;
                        sync_directory(&self.layout.uploads_dir())?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        fs::rename(&part_path, &blob_path).map_err(storage_failure)?;
                        sync_directory(blob_path.parent().expect("blob directory"))?;
                        sync_directory(&self.layout.uploads_dir())?;
                    }
                    Err(error) => return Err(storage_failure(error)),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                verify_reader(File::open(&blob_path).map_err(storage_failure)?, artifact)?;
            }
            Err(error) => return Err(storage_failure(error)),
        }
        Ok(())
    }

    pub(super) fn abort(&self, upload_id: &ArtifactUploadId) -> Result<(), ArtifactStoreError> {
        self.locked(|| {
            let mut manifest = self.load_upload(upload_id)?;
            match manifest.state {
                LocalUploadState::Committed(_) => return Err(ArtifactStoreError::UploadCommitted),
                LocalUploadState::Aborted => return Ok(()),
                LocalUploadState::Active => {}
            }
            let part_path = self.layout.upload_part(upload_id);
            match fs::remove_file(&part_path) {
                Ok(()) => sync_directory(&self.layout.uploads_dir())?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let artifact = Self::artifact_for(&manifest)?;
                    match self.load_record(artifact.artifact_id()) {
                        Ok(existing) if existing.artifact == artifact => {
                            return Err(ArtifactStoreError::UploadCommitted);
                        }
                        Ok(_) => return Err(ArtifactStoreError::IdempotencyConflict),
                        Err(ArtifactStoreError::NotFound) => {}
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => return Err(storage_failure(error)),
            }
            manifest.state = LocalUploadState::Aborted;
            write_json_atomic(&self.layout.upload_manifest(upload_id), &manifest)
        })
    }

    pub(super) fn get(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<ArtifactRecord, ArtifactStoreError> {
        self.locked(|| self.load_record(artifact_id))
    }

    pub(super) fn list(
        &self,
        after: Option<&ArtifactId>,
        limit: ArtifactPageLimit,
    ) -> Result<ArtifactPage, ArtifactStoreError> {
        self.locked(|| {
            let mut records = Vec::new();
            for entry in fs::read_dir(self.layout.artifacts_dir()).map_err(storage_failure)? {
                let path = entry.map_err(storage_failure)?.path();
                if path.extension().and_then(|value| value.to_str()) == Some("json") {
                    records.push(
                        read_json::<ArtifactRecord>(&path)?
                            .ok_or(ArtifactStoreError::StorageUnavailable)?,
                    );
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
        })
    }

    pub(super) fn read(
        &self,
        request: &ReadArtifactChunk,
        include_tombstoned: bool,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        self.locked(|| {
            let record = self.load_record(&request.artifact_id)?;
            if !include_tombstoned && record.tombstone.is_some() {
                return Err(ArtifactStoreError::Tombstoned);
            }
            let size = record.artifact.size_bytes().get();
            let offset = request.offset.get();
            if offset > size {
                return Err(unexpected_offset(size, offset));
            }
            let mut file =
                File::open(self.layout.blob(&record.artifact)).map_err(storage_failure)?;
            file.seek(SeekFrom::Start(offset))
                .map_err(storage_failure)?;
            let amount = (size - offset).min(u64::from(request.max_bytes.get())) as usize;
            let mut bytes = vec![0; amount];
            file.read_exact(&mut bytes).map_err(storage_failure)?;
            let next_offset = offset + amount as u64;
            Ok(ArtifactChunkPage {
                chunk_digest: digest_bytes(&bytes),
                bytes,
                next_offset: ArtifactByteOffset::new(next_offset),
                completion: if next_offset == size {
                    ArtifactReadCompletion::Complete
                } else {
                    ArtifactReadCompletion::More
                },
            })
        })
    }

    pub(super) fn tombstone(
        &self,
        command: TombstoneArtifact,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        self.tombstone_authorized(command, None)
    }

    pub(super) fn tombstone_authorized(
        &self,
        command: TombstoneArtifact,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        self.locked(|| {
            let mut record = self.load_record(&command.artifact_id)?;
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
            write_json_atomic(&self.layout.artifact(&command.artifact_id), &record)?;
            Ok(tombstone)
        })
    }

    fn load_upload(
        &self,
        id: &ArtifactUploadId,
    ) -> Result<LocalUploadManifest, ArtifactStoreError> {
        read_json(&self.layout.upload_manifest(id))?.ok_or(ArtifactStoreError::NotFound)
    }

    pub(super) fn artifact_id_for_upload(
        &self,
        id: &ArtifactUploadId,
    ) -> Result<ArtifactId, ArtifactStoreError> {
        Ok(Self::artifact_for(&self.load_upload(id)?)?
            .artifact_id()
            .clone())
    }

    pub(super) fn load_record(
        &self,
        id: &ArtifactId,
    ) -> Result<ArtifactRecord, ArtifactStoreError> {
        read_json(&self.layout.artifact(id))?.ok_or(ArtifactStoreError::NotFound)
    }

    fn artifact_for(manifest: &LocalUploadManifest) -> Result<ArtifactRef, ArtifactStoreError> {
        let artifact_id = match &manifest.request.requested_artifact_id {
            Some(requested) => requested.clone(),
            None => ArtifactId::new(format!(
                "artifact-{}",
                manifest.upload_id.as_str().trim_start_matches("upload-")
            ))?,
        };
        Ok(ArtifactRef::new(
            artifact_id,
            manifest.request.expected_digest.clone(),
            manifest.request.size_bytes,
            manifest.request.media_type.clone(),
            manifest.request.provenance.clone(),
        ))
    }

    fn status(
        &self,
        manifest: &LocalUploadManifest,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        let next_offset = match &manifest.state {
            LocalUploadState::Active => {
                match fs::metadata(self.layout.upload_part(&manifest.upload_id)) {
                    Ok(metadata) => metadata.len(),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        let artifact = Self::artifact_for(manifest)?;
                        verify_reader(
                            File::open(self.layout.blob(&artifact)).map_err(storage_failure)?,
                            &artifact,
                        )?;
                        artifact.size_bytes().get()
                    }
                    Err(error) => return Err(storage_failure(error)),
                }
            }
            LocalUploadState::Committed(artifact) => artifact.size_bytes().get(),
            LocalUploadState::Aborted => 0,
        };
        Ok(ArtifactUploadStatus {
            upload_id: manifest.upload_id.clone(),
            next_offset: ArtifactByteOffset::new(next_offset),
            chunk_limit: ArtifactChunkLimit::MAX,
        })
    }
}

pub(super) fn verify_reader(file: File, artifact: &ArtifactRef) -> Result<(), ArtifactStoreError> {
    let (digest, size) = digest_reader(file).map_err(storage_failure)?;
    if size != artifact.size_bytes().get() {
        return Err(ArtifactStoreError::Incomplete {
            expected: artifact.size_bytes().get(),
            actual: size,
        });
    }
    if digest != *artifact.digest() {
        return Err(ArtifactStoreError::FinalDigestMismatch);
    }
    Ok(())
}

fn unexpected_offset(expected: u64, actual: u64) -> ArtifactStoreError {
    ArtifactStoreError::UnexpectedOffset { expected, actual }
}
