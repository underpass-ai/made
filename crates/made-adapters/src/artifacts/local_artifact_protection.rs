use std::fs::{self, File};

use made_core::ports::{ArtifactIdempotencyKey, ArtifactSnapshot, ArtifactStoreError};
use made_core::value_objects::{ArtifactDigest, ArtifactId};

use super::local_artifact_io::{read_json, storage_failure, write_json_atomic};
use super::local_artifact_repository::{verify_reader, LocalArtifactRepository};

impl LocalArtifactRepository {
    pub(super) fn active_protections(&self) -> Result<Vec<ArtifactSnapshot>, ArtifactStoreError> {
        self.locked(|| self.active_protections_locked())
    }

    pub(super) fn active_protections_locked(
        &self,
    ) -> Result<Vec<ArtifactSnapshot>, ArtifactStoreError> {
        let mut snapshots = Vec::new();
        for entry in fs::read_dir(self.layout.protections_dir()).map_err(storage_failure)? {
            let path = entry.map_err(storage_failure)?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let snapshot = read_json::<ArtifactSnapshot>(&path)?
                .ok_or(ArtifactStoreError::StorageUnavailable)?;
            if snapshot.is_protected() {
                snapshots.push(snapshot);
            }
        }
        snapshots.sort_by(|left, right| left.key.as_str().cmp(right.key.as_str()));
        Ok(snapshots)
    }
    pub(super) fn protect_restore(
        &self,
        key: ArtifactIdempotencyKey,
        mut records: Vec<made_core::ports::ArtifactRecord>,
    ) -> Result<ArtifactSnapshot, ArtifactStoreError> {
        self.locked(|| {
            records.sort_by(|a, b| a.artifact.artifact_id().cmp(b.artifact.artifact_id()));
            if records
                .windows(2)
                .any(|w| w[0].artifact.artifact_id() == w[1].artifact.artifact_id())
            {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            let path = self.layout.protection(key.as_str());
            let snapshot = ArtifactSnapshot::protected(key, records);
            if let Some(existing) = read_json::<ArtifactSnapshot>(&path)? {
                if existing.key != snapshot.key || existing.records != snapshot.records {
                    return Err(ArtifactStoreError::IdempotencyConflict);
                }
                return Ok(existing);
            }
            for record in &snapshot.records {
                match self.load_record(record.artifact.artifact_id()) {
                    Ok(existing) if existing.artifact != record.artifact => {
                        return Err(ArtifactStoreError::IdempotencyConflict)
                    }
                    Ok(_) | Err(ArtifactStoreError::NotFound) => {}
                    Err(error) => return Err(error),
                }
            }
            write_json_atomic(&path, &snapshot)?;
            Ok(snapshot)
        })
    }
    pub(super) fn protect(
        &self,
        key: ArtifactIdempotencyKey,
        ids: Option<Vec<ArtifactId>>,
    ) -> Result<ArtifactSnapshot, ArtifactStoreError> {
        self.locked(|| self.protect_locked(key, ids))
    }

    /// Called while holding the artifact barrier, before taking a DB snapshot.
    pub(super) fn protect_locked(
        &self,
        key: ArtifactIdempotencyKey,
        ids: Option<Vec<ArtifactId>>,
    ) -> Result<ArtifactSnapshot, ArtifactStoreError> {
        let path = self.layout.protection(key.as_str());
        let ids = ids.map(|mut ids| {
            ids.sort();
            ids.dedup();
            ids
        });
        if let Some(existing) = read_json::<ArtifactSnapshot>(&path)? {
            if existing.key != key || existing.is_released() {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            if let Some(ids) = ids {
                let stored = existing
                    .records
                    .iter()
                    .map(|r| r.artifact.artifact_id().clone())
                    .collect::<Vec<_>>();
                if stored != ids {
                    return Err(ArtifactStoreError::IdempotencyConflict);
                }
            }
            self.verify_snapshot_content(&existing)?;
            return Ok(existing);
        }
        let mut records = match ids {
            Some(ids) => ids
                .iter()
                .map(|id| self.load_record(id))
                .collect::<Result<Vec<_>, _>>()?,
            None => self.all_records()?,
        };
        records.sort_by(|a, b| a.artifact.artifact_id().cmp(b.artifact.artifact_id()));
        let snapshot = ArtifactSnapshot::protected(key, records);
        self.verify_snapshot_content(&snapshot)?;
        write_json_atomic(&path, &snapshot)?;
        Ok(snapshot)
    }

    fn verify_snapshot_content(
        &self,
        snapshot: &ArtifactSnapshot,
    ) -> Result<(), ArtifactStoreError> {
        for record in &snapshot.records {
            match File::open(self.layout.blob(&record.artifact)) {
                Ok(file) => verify_reader(file, &record.artifact)?,
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && record.tombstone.is_some() => {}
                Err(error) => return Err(storage_failure(error)),
            }
        }
        Ok(())
    }

    pub(super) fn release_protection(
        &self,
        key: &ArtifactIdempotencyKey,
    ) -> Result<(), ArtifactStoreError> {
        self.locked(|| {
            let path = self.layout.protection(key.as_str());
            let mut snapshot =
                read_json::<ArtifactSnapshot>(&path)?.ok_or(ArtifactStoreError::NotFound)?;
            if snapshot.key != *key {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            snapshot.release();
            write_json_atomic(&path, &snapshot)
        })
    }

    /// Must be checked under the same lock as unlink; corrupt pins fail closed.
    pub(super) fn is_protected(&self, digest: &ArtifactDigest) -> Result<bool, ArtifactStoreError> {
        for entry in fs::read_dir(self.layout.protections_dir()).map_err(storage_failure)? {
            let path = entry.map_err(storage_failure)?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let snapshot = read_json::<ArtifactSnapshot>(&path)?
                .ok_or(ArtifactStoreError::StorageUnavailable)?;
            if snapshot.is_protected()
                && snapshot
                    .records
                    .iter()
                    .any(|record| record.artifact.digest() == digest)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
