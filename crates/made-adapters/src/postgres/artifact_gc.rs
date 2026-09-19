use std::collections::{BTreeMap, BTreeSet};

use made_core::ports::{ArtifactRecord, ArtifactSnapshot, ArtifactStoreError, BeginArtifactUpload};
use made_core::value_objects::ArtifactDigest;
use sqlx::{Postgres, Row, Transaction};

use crate::artifacts::{
    ArtifactGcCandidate, ArtifactGcExclusion, ArtifactGcExclusionReason, ArtifactGcPlan,
    ArtifactGcReport,
};

use super::artifact_store::{storage_failure, PostgresArtifactStore};

impl PostgresArtifactStore {
    pub async fn plan_gc(
        &self,
        retire_before: time::OffsetDateTime,
        lease: made_core::value_objects::StepLease,
    ) -> Result<ArtifactGcPlan, ArtifactStoreError> {
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        Self::lock_protection_barrier(&mut tx).await?;
        let records = all_records(&mut tx).await?;
        let active = active_upload_digests(&mut tx).await?;
        let protected = protected_digests(&mut tx).await?;
        let mut groups: BTreeMap<ArtifactDigest, Vec<ArtifactRecord>> = BTreeMap::new();
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
            if active.contains(&digest) {
                reasons.push(ArtifactGcExclusionReason::ActiveUpload);
            }
            if protected.contains(&digest) {
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
            let bytes = blob_bytes(&mut tx, &digest).await?;
            if bytes == 0 {
                exclusions.push(ArtifactGcExclusion {
                    digest,
                    artifact_ids,
                    reasons: vec![ArtifactGcExclusionReason::NoReclaimableBytes],
                });
                continue;
            }
            candidates.push(ArtifactGcCandidate {
                digest,
                bytes,
                artifact_ids: records
                    .into_iter()
                    .map(|record| record.artifact.artifact_id().clone())
                    .collect(),
            });
        }
        tx.commit().await.map_err(storage_failure)?;
        Ok(ArtifactGcPlan {
            version: 1,
            retire_before,
            lease,
            candidates,
            exclusions,
        })
    }

    pub async fn apply_gc(
        &self,
        plan: &ArtifactGcPlan,
        now: time::OffsetDateTime,
    ) -> Result<ArtifactGcReport, ArtifactStoreError> {
        if plan.version != 1 || plan.lease.is_expired_at(now) {
            return Err(ArtifactStoreError::AccessDenied);
        }
        let mut tx = self.pool.inner().begin().await.map_err(storage_failure)?;
        Self::lock_protection_barrier(&mut tx).await?;
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
            if plan.lease.is_expired_at(time::OffsetDateTime::now_utc()) {
                return Err(ArtifactStoreError::AccessDenied);
            }
            let mut matching = all_records(&mut tx)
                .await?
                .into_iter()
                .filter(|record| record.artifact.digest() == &candidate.digest)
                .collect::<Vec<_>>();
            matching.sort_by(|left, right| {
                left.artifact
                    .artifact_id()
                    .cmp(right.artifact.artifact_id())
            });
            let ids = matching
                .iter()
                .map(|record| record.artifact.artifact_id().clone())
                .collect::<Vec<_>>();
            if ids != candidate.artifact_ids
                || matching.is_empty()
                || matching.iter().any(|record| {
                    record
                        .tombstone
                        .as_ref()
                        .is_none_or(|tombstone| tombstone.retired_at > plan.retire_before)
                })
                || active_upload_digests(&mut tx)
                    .await?
                    .contains(&candidate.digest)
                || protected_digests(&mut tx)
                    .await?
                    .contains(&candidate.digest)
            {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            let bytes = blob_bytes(&mut tx, &candidate.digest).await?;
            if bytes == 0 {
                continue;
            }
            sqlx::query("DELETE FROM artifact_blobs WHERE digest = $1")
                .bind(candidate.digest.as_str())
                .execute(&mut *tx)
                .await
                .map_err(storage_failure)?;
            report.reclaimed_bytes = report.reclaimed_bytes.saturating_add(bytes);
            report.deleted.push(candidate.digest.clone());
        }
        tx.commit().await.map_err(storage_failure)?;
        Ok(report)
    }
}

async fn all_records(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<ArtifactRecord>, ArtifactStoreError> {
    sqlx::query("SELECT body FROM artifact_records ORDER BY artifact_id FOR SHARE")
        .fetch_all(&mut **tx)
        .await
        .map_err(storage_failure)?
        .into_iter()
        .map(|row| {
            serde_json::from_value(row.try_get("body").map_err(storage_failure)?)
                .map_err(|_| ArtifactStoreError::StorageUnavailable)
        })
        .collect()
}

async fn active_upload_digests(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<BTreeSet<ArtifactDigest>, ArtifactStoreError> {
    sqlx::query("SELECT request FROM artifact_uploads WHERE state = 'active'")
        .fetch_all(&mut **tx)
        .await
        .map_err(storage_failure)?
        .into_iter()
        .map(|row| {
            serde_json::from_value::<BeginArtifactUpload>(
                row.try_get("request").map_err(storage_failure)?,
            )
            .map(|request| request.expected_digest)
            .map_err(|_| ArtifactStoreError::StorageUnavailable)
        })
        .collect()
}

async fn protected_digests(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<BTreeSet<ArtifactDigest>, ArtifactStoreError> {
    let snapshots: Vec<ArtifactSnapshot> =
        sqlx::query("SELECT body FROM artifact_protections WHERE state = 'protected' FOR SHARE")
            .fetch_all(&mut **tx)
            .await
            .map_err(storage_failure)?
            .into_iter()
            .map(|row| {
                serde_json::from_value(row.try_get("body").map_err(storage_failure)?)
                    .map_err(|_| ArtifactStoreError::StorageUnavailable)
            })
            .collect::<Result<_, _>>()?;
    Ok(snapshots
        .into_iter()
        .flat_map(|snapshot| {
            snapshot
                .records
                .into_iter()
                .map(|record| record.artifact.digest().clone())
        })
        .collect())
}

async fn blob_bytes(
    tx: &mut Transaction<'_, Postgres>,
    digest: &ArtifactDigest,
) -> Result<u64, ArtifactStoreError> {
    let bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(OCTET_LENGTH(bytes)), 0)::BIGINT FROM artifact_blobs WHERE digest = $1",
    )
    .bind(digest.as_str())
    .fetch_one(&mut **tx)
    .await
    .map_err(storage_failure)?;
    u64::try_from(bytes).map_err(|_| ArtifactStoreError::StorageUnavailable)
}
