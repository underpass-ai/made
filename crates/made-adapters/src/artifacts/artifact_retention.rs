use std::sync::Arc;

use made_core::ports::ArtifactTombstone;
use made_core::ports::{
    ArtifactPageLimit, ArtifactRecord, ArtifactRetentionActor, ArtifactRetentionPolicy,
    ArtifactStoreError, ArtifactStorePort, TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactId, AuthorizationEvidence, DurationMs, IdempotencyKey, LeaseOwnerId, StepLease,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// A reviewed retention selection. Nothing is retired by constructing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRetentionPlan {
    pub version: u32,
    pub observed_before: OffsetDateTime,
    pub retired_at: OffsetDateTime,
    pub actor: ArtifactRetentionActor,
    pub policy: ArtifactRetentionPolicy,
    pub lease: StepLease,
    pub records: Vec<ArtifactRecord>,
}

/// Audit result for either a dry-run or an explicit retention apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRetentionReport {
    pub dry_run: bool,
    pub planned: Vec<ArtifactId>,
    pub applied: Vec<ArtifactTombstone>,
    pub already_retired: Vec<ArtifactId>,
}

impl ArtifactRetentionReport {
    pub(super) fn dry_run(plan: &ArtifactRetentionPlan) -> Self {
        Self {
            dry_run: true,
            planned: plan
                .records
                .iter()
                .map(|record| record.artifact.artifact_id().clone())
                .collect(),
            applied: Vec::new(),
            already_retired: Vec::new(),
        }
    }
}

/// Retention planner over the existing artifact store boundary.
#[derive(Debug)]
pub struct ArtifactRetentionService<S> {
    store: Arc<S>,
}

impl<S> ArtifactRetentionService<S>
where
    S: ArtifactStorePort + 'static,
{
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// The caller supplies the clock and lease so plans are deterministic and
    /// can be reviewed or persisted before any tombstone is written.
    pub async fn plan_retention(
        &self,
        actor: ArtifactRetentionActor,
        policy: ArtifactRetentionPolicy,
        observed_before: OffsetDateTime,
        retired_at: OffsetDateTime,
        lease: StepLease,
    ) -> Result<ArtifactRetentionPlan, ArtifactStoreError> {
        let mut records = Vec::new();
        let mut after = None;
        loop {
            let page = self
                .store
                .list(after.as_ref(), ArtifactPageLimit::default())
                .await?;
            records.extend(page.items.into_iter().filter(|record| {
                record.tombstone.is_none()
                    && record.artifact.provenance().observed_at() < observed_before
            }));
            match page.next_after {
                Some(cursor) => after = Some(cursor),
                None => break,
            }
        }
        records.sort_by(|left, right| {
            left.artifact
                .artifact_id()
                .cmp(right.artifact.artifact_id())
        });
        Ok(ArtifactRetentionPlan {
            version: 1,
            observed_before,
            retired_at,
            actor,
            policy,
            lease,
            records,
        })
    }

    /// Convenience constructor for a lease using the existing ceremony lease
    /// value object rather than introducing another clock/fencing primitive.
    pub fn lease(
        owner: LeaseOwnerId,
        idempotency_key: IdempotencyKey,
        acquired_at: OffsetDateTime,
        ttl: DurationMs,
    ) -> Result<StepLease, ArtifactStoreError> {
        StepLease::acquire(owner, idempotency_key, acquired_at, ttl).map_err(Into::into)
    }

    #[must_use]
    pub fn dry_run(plan: &ArtifactRetentionPlan) -> ArtifactRetentionReport {
        ArtifactRetentionReport::dry_run(plan)
    }

    /// Apply an already reviewed plan. The lease is checked for every item;
    /// retries keep the first tombstone and never erase its authorization.
    pub async fn apply(
        &self,
        plan: &ArtifactRetentionPlan,
        now: OffsetDateTime,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactRetentionReport, ArtifactStoreError> {
        if plan.version != 1 || plan.lease.is_expired_at(now) {
            return Err(ArtifactStoreError::AccessDenied);
        }
        let mut report = ArtifactRetentionReport {
            dry_run: false,
            planned: plan
                .records
                .iter()
                .map(|record| record.artifact.artifact_id().clone())
                .collect(),
            applied: Vec::new(),
            already_retired: Vec::new(),
        };
        for record in &plan.records {
            if plan.lease.is_expired_at(now) {
                return Err(ArtifactStoreError::AccessDenied);
            }
            let current = self.store.get(record.artifact.artifact_id()).await?;
            if current.artifact != record.artifact {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            if current.tombstone.is_some() {
                report
                    .already_retired
                    .push(record.artifact.artifact_id().clone());
                continue;
            }
            let tombstone = self
                .store
                .tombstone_authorized(
                    TombstoneArtifact {
                        artifact_id: record.artifact.artifact_id().clone(),
                        actor: plan.actor.clone(),
                        policy: plan.policy.clone(),
                        retired_at: plan.retired_at,
                    },
                    authorization.clone(),
                )
                .await?;
            report.applied.push(tombstone);
        }
        Ok(report)
    }
}

// Keep this import path checked by the compiler while the public plan uses
// the canonical core record/tombstone types.
#[allow(unused_imports)]
use made_core::ports::ArtifactStorePort as _;
