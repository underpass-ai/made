#![cfg(feature = "sqlite")]

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use made_adapters::artifacts::{
    ArtifactGcExclusionReason, LocalArtifactStore, SqliteArtifactBackupService,
};
use made_app::artifacts::ArtifactService;
use made_core::ports::{
    ArtifactIdempotencyKey, ArtifactRetentionActor, ArtifactRetentionPolicy, ArtifactStorePort,
    TombstoneArtifact,
};
use made_core::value_objects::{
    ArtifactMediaType, ArtifactRef, DurationMs, IdempotencyKey, LeaseOwnerId, StepLease,
};
use tempfile::TempDir;
use time::OffsetDateTime;

async fn artifact(store: &LocalArtifactStore, bytes: &[u8], key: &str) -> ArtifactRef {
    ArtifactService::new(Arc::new(store.clone()))
        .save_generated_report(
            bytes,
            ArtifactMediaType::new("application/octet-stream").unwrap(),
            OffsetDateTime::now_utc(),
            ArtifactIdempotencyKey::new(key).unwrap(),
        )
        .await
        .unwrap()
}
fn lease() -> StepLease {
    StepLease::acquire(
        LeaseOwnerId::new("gc-acceptance").unwrap(),
        IdempotencyKey::new("gc-acceptance").unwrap(),
        OffsetDateTime::now_utc(),
        DurationMs::from_millis(60_000),
    )
    .unwrap()
}
async fn retire(store: &LocalArtifactStore, artifact: &ArtifactRef) {
    store
        .tombstone(TombstoneArtifact {
            artifact_id: artifact.artifact_id().clone(),
            actor: ArtifactRetentionActor::new("test").unwrap(),
            policy: ArtifactRetentionPolicy::new("acceptance").unwrap(),
            retired_at: OffsetDateTime::UNIX_EPOCH,
        })
        .await
        .unwrap();
}
fn directory() -> TempDir {
    std::fs::create_dir_all("tmp").unwrap();
    TempDir::new_in("tmp").unwrap()
}

#[tokio::test]
async fn gc_preview_explains_live_and_protected_references() {
    let root = directory();
    let store = LocalArtifactStore::open(root.path()).unwrap();
    let live = artifact(&store, b"live", "live").await;
    let pinned = artifact(&store, b"pinned", "pinned").await;
    retire(&store, &pinned).await;
    store
        .protect_references(
            ArtifactIdempotencyKey::new("receipt:preview").unwrap(),
            vec![pinned.artifact_id().clone()],
        )
        .await
        .unwrap();
    let plan = store
        .plan_gc(OffsetDateTime::now_utc(), lease())
        .await
        .unwrap();
    assert!(plan.candidates.is_empty());
    assert!(plan.exclusions.iter().any(|e| e.digest == *live.digest()
        && e.reasons
            .contains(&ArtifactGcExclusionReason::LiveReference)));
    assert!(plan.exclusions.iter().any(|e| e.digest == *pinned.digest()
        && e.reasons
            .contains(&ArtifactGcExclusionReason::ProtectedReference)));
}

#[tokio::test]
async fn gc_resumes_after_a_partial_failure_without_double_counting_bytes() {
    let root = directory();
    let store = LocalArtifactStore::open(root.path()).unwrap();
    let first = artifact(&store, b"first blob", "first").await;
    let second = artifact(&store, b"second blob", "second").await;
    retire(&store, &first).await;
    retire(&store, &second).await;
    let plan = store
        .plan_gc(OffsetDateTime::now_utc(), lease())
        .await
        .unwrap();
    assert_eq!(plan.candidates.len(), 2);
    let last = &plan.candidates[1];
    let last_path = root
        .path()
        .join("blobs")
        .join(last.digest.as_str().trim_start_matches("sha256:"));
    let original = std::fs::read(&last_path).unwrap();
    // Deterministic fault after plan verification: first unlink succeeds, then
    // the second candidate fails integrity verification.
    std::fs::write(&last_path, b"fault injected").unwrap();
    assert!(store
        .apply_gc(&plan, OffsetDateTime::now_utc())
        .await
        .is_err());
    assert!(!root
        .path()
        .join("blobs")
        .join(
            plan.candidates[0]
                .digest
                .as_str()
                .trim_start_matches("sha256:")
        )
        .exists());
    std::fs::write(&last_path, original).unwrap();
    let reopened = LocalArtifactStore::open(root.path()).unwrap();
    let resumed = reopened
        .apply_gc(&plan, OffsetDateTime::now_utc())
        .await
        .unwrap();
    assert_eq!(resumed.deleted, vec![last.digest.clone()]);
    assert_eq!(resumed.reclaimed_bytes, last.bytes);
    let repeated = reopened
        .apply_gc(&plan, OffsetDateTime::now_utc())
        .await
        .unwrap();
    assert!(repeated.deleted.is_empty());
    assert_eq!(repeated.reclaimed_bytes, 0);
    assert!(reopened
        .get(first.artifact_id())
        .await
        .unwrap()
        .tombstone
        .is_some());
    assert!(reopened
        .get(second.artifact_id())
        .await
        .unwrap()
        .tombstone
        .is_some());
}

#[tokio::test]
async fn measures_sqlite_recovery_point_and_time_with_a_live_writer() {
    let root = directory();
    let database = root.path().join("live.sqlite3");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE checkpoints(seq INTEGER PRIMARY KEY, observed_us INTEGER NOT NULL);").unwrap();
    drop(connection);
    let store = LocalArtifactStore::open(root.path().join("artifacts")).unwrap();
    let mut refs = Vec::new();
    for index in 0..16 {
        refs.push(artifact(&store, &vec![index; 65_536], &format!("load-{index}")).await);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let count = Arc::new(AtomicU64::new(0));
    let writer_stop = stop.clone();
    let writer_count = count.clone();
    let writer = tokio::task::spawn_blocking(move || {
        let connection = rusqlite::Connection::open(database).unwrap();
        connection.busy_timeout(Duration::from_secs(5)).unwrap();
        for sequence in 1..=2000_u64 {
            if writer_stop.load(Ordering::Acquire) {
                break;
            }
            let observed_us = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1000;
            connection
                .execute(
                    "INSERT INTO checkpoints VALUES (?1,?2)",
                    rusqlite::params![
                        i64::try_from(sequence).unwrap(),
                        i64::try_from(observed_us).unwrap()
                    ],
                )
                .unwrap();
            writer_count.store(sequence, Ordering::Release);
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    while count.load(Ordering::Acquire) < 5 {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let before = count.load(Ordering::Acquire);
    let service = SqliteArtifactBackupService::new(store, root.path().join("live.sqlite3"));
    let backup_started = Instant::now();
    let backup = root.path().join("backup");
    let manifest = service
        .backup_to(
            &backup,
            ArtifactIdempotencyKey::new("backup:measured").unwrap(),
        )
        .await
        .unwrap();
    let backup_duration = backup_started.elapsed();
    stop.store(true, Ordering::Release);
    writer.await.unwrap();
    let incident_at = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1000;
    service.verify(&backup, &manifest).unwrap();
    let restore_started = Instant::now();
    let restored = root.path().join("isolated-restore");
    service.restore_set_to(&backup, &restored).await.unwrap();
    let restored_db = rusqlite::Connection::open(restored.join("database.sqlite3")).unwrap();
    let (restored_sequence, last_us): (i64, i64) = restored_db
        .query_row(
            "SELECT seq,observed_us FROM checkpoints ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let restored_sequence = u64::try_from(restored_sequence).unwrap();
    let restored_store = LocalArtifactStore::open(restored.join("artifacts")).unwrap();
    for artifact in refs {
        assert!(restored_store
            .backup_content_available(artifact.artifact_id())
            .await
            .unwrap());
    }
    let recovery_duration = restore_started.elapsed();
    let last_committed = count.load(Ordering::Acquire);
    assert!(restored_sequence >= before && restored_sequence <= last_committed);
    assert!(
        last_committed > before,
        "writer must progress during backup"
    );
    println!(
        "{}",
        serde_json::json!({"backend":"sqlite","sample":"one_local_backup_with_writer_and_1MiB_blobs","backup_ms":backup_duration.as_secs_f64()*1000.0,"rto_ms":recovery_duration.as_secs_f64()*1000.0,"rpo_ms":(incident_at-i128::from(last_us)) as f64/1000.0,"head_before_backup":before,"restored_sequence":restored_sequence,"head_at_incident":last_committed,"lost_commits":last_committed-restored_sequence})
    );
}
