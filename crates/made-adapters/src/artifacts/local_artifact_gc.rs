use super::local_artifact_io::{read_json, storage_failure, sync_directory};
use super::local_artifact_repository::{verify_reader, LocalArtifactRepository};
use super::local_upload_manifest::LocalUploadManifest;
use super::local_upload_state::LocalUploadState;
use super::{
    ArtifactGcCandidate, ArtifactGcExclusion, ArtifactGcExclusionReason, ArtifactGcPlan,
    ArtifactGcReport,
};
use made_core::ports::{ArtifactRecord, ArtifactStoreError};
use std::fs::{self, File};

impl LocalArtifactRepository {
    pub(super) fn plan_gc(
        &self,
        retire_before: time::OffsetDateTime,
        lease: made_core::value_objects::StepLease,
    ) -> Result<ArtifactGcPlan, ArtifactStoreError> {
        self.locked(|| {
            let records = self.all_records()?;
            let mut groups: std::collections::BTreeMap<_, Vec<ArtifactRecord>> =
                std::collections::BTreeMap::new();
            for record in records {
                groups
                    .entry(record.artifact.digest().clone())
                    .or_default()
                    .push(record);
            }
            let mut candidates = Vec::new();
            let mut exclusions = Vec::new();
            for (digest, mut records) in groups {
                records.sort_by(|left, right| {
                    left.artifact
                        .artifact_id()
                        .cmp(right.artifact.artifact_id())
                });
                let mut reasons = Vec::new();
                if records.iter().any(|record| record.tombstone.is_none()) {
                    reasons.push(ArtifactGcExclusionReason::LiveReference);
                }
                if records.iter().any(|record| {
                    record
                        .tombstone
                        .as_ref()
                        .is_some_and(|t| t.retired_at > retire_before)
                }) {
                    reasons.push(ArtifactGcExclusionReason::RetentionWindow);
                }
                if self.has_active_upload_for(&digest)? {
                    reasons.push(ArtifactGcExclusionReason::ActiveUpload);
                }
                if self.is_protected(&digest)? {
                    reasons.push(ArtifactGcExclusionReason::ProtectedReference);
                }
                let artifact_ids = records
                    .iter()
                    .map(|record| record.artifact.artifact_id().clone())
                    .collect();
                if !reasons.is_empty() {
                    exclusions.push(ArtifactGcExclusion {
                        digest,
                        artifact_ids,
                        reasons,
                    });
                    continue;
                }
                let path = self.layout.blob(&records[0].artifact);
                let metadata = match fs::metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        exclusions.push(ArtifactGcExclusion {
                            digest,
                            artifact_ids,
                            reasons: vec![ArtifactGcExclusionReason::NoReclaimableBytes],
                        });
                        continue;
                    }
                    Err(error) => return Err(storage_failure(error)),
                };
                verify_reader(
                    File::open(&path).map_err(storage_failure)?,
                    &records[0].artifact,
                )?;
                if metadata.len() == 0 {
                    exclusions.push(ArtifactGcExclusion {
                        digest,
                        artifact_ids,
                        reasons: vec![ArtifactGcExclusionReason::NoReclaimableBytes],
                    });
                    continue;
                }
                candidates.push(ArtifactGcCandidate {
                    digest,
                    bytes: metadata.len(),
                    artifact_ids: records
                        .into_iter()
                        .map(|record| record.artifact.artifact_id().clone())
                        .collect(),
                });
            }
            candidates.sort_by(|left, right| left.digest.cmp(&right.digest));
            Ok(ArtifactGcPlan {
                version: 1,
                retire_before,
                lease,
                candidates,
                exclusions,
            })
        })
    }

    pub(super) fn apply_gc(
        &self,
        plan: &ArtifactGcPlan,
        now: time::OffsetDateTime,
    ) -> Result<ArtifactGcReport, ArtifactStoreError> {
        self.locked(|| {
            if plan.version != 1 || plan.lease.is_expired_at(now) {
                return Err(ArtifactStoreError::AccessDenied);
            }
            let mut report = ArtifactGcReport {
                dry_run: false,
                planned: plan
                    .candidates
                    .iter()
                    .map(|candidate| candidate.digest.clone())
                    .collect(),
                deleted: Vec::new(),
                reclaimed_bytes: 0,
            };
            for candidate in &plan.candidates {
                if plan.lease.is_expired_at(now) {
                    return Err(ArtifactStoreError::AccessDenied);
                }
                let records = self.all_records()?;
                let mut matching = records
                    .iter()
                    .filter(|record| record.artifact.digest() == &candidate.digest)
                    .collect::<Vec<_>>();
                matching.sort_by(|left, right| {
                    left.artifact
                        .artifact_id()
                        .cmp(right.artifact.artifact_id())
                });
                let current_ids = matching
                    .iter()
                    .map(|record| record.artifact.artifact_id().clone())
                    .collect::<Vec<_>>();
                if current_ids != candidate.artifact_ids
                    || matching.is_empty()
                    || matching.iter().any(|record| {
                        record
                            .tombstone
                            .as_ref()
                            .is_none_or(|tombstone| tombstone.retired_at > plan.retire_before)
                    })
                    || self.has_active_upload_for(&candidate.digest)?
                    || self.is_protected(&candidate.digest)?
                {
                    return Err(ArtifactStoreError::IdempotencyConflict);
                }
                let path = self.layout.blob(&matching[0].artifact);
                if !path.exists() {
                    continue;
                }
                verify_reader(
                    File::open(&path).map_err(storage_failure)?,
                    &matching[0].artifact,
                )?;
                let bytes = fs::metadata(&path).map_err(storage_failure)?.len();
                fs::remove_file(path).map_err(storage_failure)?;
                report.reclaimed_bytes = report.reclaimed_bytes.saturating_add(bytes);
                sync_directory(&self.layout.blobs_dir())?;
                report.deleted.push(candidate.digest.clone());
            }
            Ok(report)
        })
    }

    pub(super) fn all_records(&self) -> Result<Vec<ArtifactRecord>, ArtifactStoreError> {
        let mut records = Vec::new();
        for entry in fs::read_dir(self.layout.artifacts_dir()).map_err(storage_failure)? {
            let path = entry.map_err(storage_failure)?.path();
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                records.push(
                    read_json::<ArtifactRecord>(&path)?
                        .ok_or_else(|| ArtifactStoreError::unavailable_static("artifact record is missing its required metadata"))?,
                );
            }
        }
        Ok(records)
    }

    fn has_active_upload_for(
        &self,
        digest: &made_core::value_objects::ArtifactDigest,
    ) -> Result<bool, ArtifactStoreError> {
        for entry in fs::read_dir(self.layout.uploads_dir()).map_err(storage_failure)? {
            let path = entry.map_err(storage_failure)?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let manifest = read_json::<LocalUploadManifest>(&path)?
                .ok_or_else(|| ArtifactStoreError::unavailable_static("artifact record is missing its required metadata"))?;
            if matches!(manifest.state, LocalUploadState::Active)
                && manifest.request.expected_digest == *digest
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
