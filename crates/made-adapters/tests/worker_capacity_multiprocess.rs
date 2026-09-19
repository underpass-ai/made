use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;

use made_adapters::memory::InMemoryCeremonyEventStore;
use made_adapters::workers::FileWorkerCapacityStore;
use made_app::services::SessionStream;
use made_app::workers::{
    CeremonyWorkerCapacity, WorkerCapacityLimits, WorkerCapacityPort, WorkerCapacityRequest,
};
use made_core::ports::NoopCeremonyEventSubscriber;
use made_core::value_objects::{
    CeremonyId, ExecutionConnectorId, ExecutionOperationId, LeaseOwnerId, StateIteration,
    StateVisit, StepId, StepIteration,
};
use time::OffsetDateTime;

const CHILD_MODE: &str = "MADE_CAPACITY_CHILD";

fn stream() -> Arc<SessionStream> {
    let events = Arc::new(InMemoryCeremonyEventStore::new());
    Arc::new(SessionStream::new(
        events.clone(),
        events,
        Arc::new(NoopCeremonyEventSubscriber),
    ))
}

fn limits() -> WorkerCapacityLimits {
    WorkerCapacityLimits {
        global: CeremonyWorkerCapacity::new(2).unwrap(),
        per_root: CeremonyWorkerCapacity::new(2).unwrap(),
        per_connector: CeremonyWorkerCapacity::new(2).unwrap(),
        per_provider: CeremonyWorkerCapacity::new(2).unwrap(),
    }
}

fn request(index: u8, now: OffsetDateTime) -> WorkerCapacityRequest {
    let ceremony = CeremonyId::new(format!("ceremony-{index}")).unwrap();
    let step = StepId::new("work").unwrap();
    WorkerCapacityRequest {
        operation_id: ExecutionOperationId::for_step(
            &ceremony,
            &step,
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
        ),
        ceremony_id: ceremony.clone(),
        step_id: step,
        root_id: CeremonyId::new("shared-root").unwrap(),
        connector_id: ExecutionConnectorId::new("oci").unwrap(),
        provider_id: ExecutionConnectorId::new("provider").unwrap(),
        owner_id: LeaseOwnerId::new(format!("owner-{index}")).unwrap(),
        pending_until: now + time::Duration::seconds(5),
    }
}

#[tokio::test]
#[ignore = "subprocess helper"]
async fn capacity_child() {
    if std::env::var(CHILD_MODE).ok().as_deref() != Some("1") {
        return;
    }
    let directory = PathBuf::from(std::env::var("MADE_CAPACITY_DIRECTORY").unwrap());
    let result = PathBuf::from(std::env::var("MADE_CAPACITY_RESULT").unwrap());
    let index = std::env::var("MADE_CAPACITY_INDEX")
        .unwrap()
        .parse::<u8>()
        .unwrap();
    let now = OffsetDateTime::now_utc();
    let admitted = FileWorkerCapacityStore::new(directory, limits(), stream())
        .unwrap()
        .reserve(&request(index, now), now)
        .await
        .unwrap();
    std::fs::write(result, if admitted { "admitted" } else { "deferred" }).unwrap();
    // Exit without release: this simulates a worker dying after reservation.
}

fn spawn_child(directory: &Path, result: &Path, index: u8) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .arg("--ignored")
        .arg("--exact")
        .arg("capacity_child")
        .arg("--test-threads=1")
        .env(CHILD_MODE, "1")
        .env("MADE_CAPACITY_DIRECTORY", directory)
        .env("MADE_CAPACITY_RESULT", result)
        .env("MADE_CAPACITY_INDEX", index.to_string())
        .spawn()
        .unwrap()
}

#[tokio::test]
async fn three_processes_share_capacity_and_recover_dead_pending_owners() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = tempfile::tempdir_in("tmp").unwrap();
    let mut children = Vec::new();
    let mut results = Vec::new();
    for index in 0..3_u8 {
        let result = directory.path().join(format!("result-{index}"));
        children.push(spawn_child(directory.path(), &result, index));
        results.push(result);
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let admitted = results
        .iter()
        .filter(|path| std::fs::read_to_string(path).unwrap() == "admitted")
        .count();
    assert_eq!(admitted, 2, "the process-shared global limit must hold");

    let after_pending_expiry = OffsetDateTime::now_utc() + time::Duration::seconds(10);
    let recovered = FileWorkerCapacityStore::new(directory.path(), limits(), stream())
        .unwrap()
        .reserve(&request(9, after_pending_expiry), after_pending_expiry)
        .await
        .unwrap();
    assert!(recovered, "dead unbound owners must not leak capacity");
}
